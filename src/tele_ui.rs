use std::io;
use std::net::UdpSocket;
use std::ops::DerefMut;

use conrod::{
    Background,
    Button,
    Color,
    Colorable,
    Frameable,
    Label,
    Labelable,
    Positionable,
    Slider,
    Sizeable,
    Ui,
    WidgetId,
    Widget,
};
use conrod::color::{rgb, white};
use graphics::Context;
use opengl_graphics::GlGraphics;
use opengl_graphics::glyph_cache::GlyphCache;
use piston::input;
use time;

use line_graph::LineGraph;

enum MissionTime {
    Paused(time::Duration),
    Running(time::Tm, time::Duration),
}

pub struct TelemetryUi {
    socket: UdpSocket,

    bg_color: Color,

    mission_time: MissionTime,

    // RPM stuff
    l_rpm_status: String,
    r_rpm_status: String,

    // Voltage stuff
    v24_graph: LineGraph,
    v_h_24: Option<(f64, f64)>,

    v12_graph: LineGraph,
    v_p_12_e: Option<(f64, f64)>,

    v_p_12_pl: Option<(f64, f64)>,

    // Motor temp
    motor_temp_graph: LineGraph,
    l_motor_temp: Option<f64>,
    r_motor_temp: Option<f64>,

    // Avionics box temp
    upper_avionics_temp: Option<f64>,
    lower_avionics_temp: Option<f64>,

    // Weather section
    wind_speed: Option<f64>,
    pressure: Option<f64>,
    altitude: Option<f64>,
    temp: Option<f64>,

    // IMU
    pitch_roll_heading: Option<(f64, f64, f64)>,
}

impl TelemetryUi {
    pub fn new(socket: UdpSocket) -> TelemetryUi {
        let v24_graph = LineGraph::new((400.0, 150.0), (0.0, 100.0), (0.0, 40.0));
        let v12_graph = LineGraph::new((400.0, 150.0), (0.0, 100.0), (0.0, 20.0));
        let motor_temp_graph = LineGraph::new((400.0, 150.0), (0.0, 100.0), (0.0, 100.0));

        TelemetryUi {
            socket: socket,

            bg_color: rgb(0.2, 0.35, 0.45),

            mission_time: MissionTime::Paused(time::Duration::zero()),

            l_rpm_status: "NO DATA".to_string(),
            r_rpm_status: "NO DATA".to_string(),

            v24_graph: v24_graph,
            v_h_24: None,

            v12_graph: v12_graph,
            v_p_12_e: None,

            v_p_12_pl: None,

            motor_temp_graph: motor_temp_graph,
            l_motor_temp: None,
            r_motor_temp: None,

            upper_avionics_temp: None,
            lower_avionics_temp: None,

            wind_speed: None,
            pressure: None,
            altitude: None,
            temp: None,

            pitch_roll_heading: None,
        }
    }

    pub fn draw_ui<'a>(&mut self, c: Context, gl: &mut GlGraphics, ui: &mut Ui<GlyphCache<'a>>) {
        use graphics::*;

        // Draw the background.
        Background::new().color(self.bg_color).draw(ui, gl);

        let time_now = time::now();

        // Local time
        Label::new(format!("{}", time_now.strftime("Local  %x  %X").unwrap()).as_str())
            .xy((-ui.win_w / 2.0) + 100.0, (ui.win_h / 2.0) - 10.0)
            .font_size(16)
            .color(self.bg_color.plain_contrast())
            .set(LOCAL_TIME, ui);

        // UTC time
        Label::new(format!("{}", time_now.to_utc().strftime("%Z  %x  %X").unwrap()).as_str())
            .xy((-ui.win_w / 2.0) + 104.0, (ui.win_h / 2.0) - 30.0)
            .font_size(16)
            .color(self.bg_color.plain_contrast())
            .set(UTC_TIME, ui);

        // Mission time label
        let mission_time =
            match self.mission_time {
                MissionTime::Paused(t) => t,
                MissionTime::Running(start_time, extra_time) =>
                    (time::now() - start_time) + extra_time
            };
        let total_days = mission_time.num_days();
        let total_hours = mission_time.num_hours();
        let total_minutes = mission_time.num_minutes();
        let total_seconds = mission_time.num_seconds();

        let days = total_days;
        let hours = total_hours - total_days*24;
        let minutes = total_minutes - total_hours*60;
        let seconds = total_seconds - total_minutes*60;
        Label::new(format!("Mission Time: {}:{}:{}:{}", days, hours, minutes, seconds).as_str())
            .xy((-ui.win_w / 2.0) + 150.0, (ui.win_h / 2.0) - 70.0)
            .font_size(20)
            .color(self.bg_color.plain_contrast())
            .set(MISSION_TIME_LABEL, ui);

        // Mission start/pause button
        let mission_start_text =
            match self.mission_time {
                MissionTime::Paused(_) => "Start",
                MissionTime::Running(_, _) => "Pause",
            };
        Button::new()
            .dimensions(100.0, 30.0)
            .xy((-ui.win_w / 2.0) + 55.0, (ui.win_h / 2.0) - 100.0)
            .rgb(0.3, 0.8, 0.3)
            .frame(1.0)
            .label(mission_start_text)
            .react(|| {
                match self.mission_time {
                    MissionTime::Paused(current_time) => {
                        self.mission_time = MissionTime::Running(time::now(), current_time);
                    },
                    MissionTime::Running(start_time, extra_time) => {
                        self.mission_time = MissionTime::Paused((time::now() - start_time) + extra_time);
                    },
                };
            })
            .set(MISSION_START_BUTTON, ui);

        // Mission reset button
        Button::new()
            .dimensions(100.0, 30.0)
            .xy((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 100.0)
            .rgb(0.3, 0.8, 0.3)
            .frame(1.0)
            .label("Reset")
            .react(|| {
                self.mission_time = MissionTime::Paused(time::Duration::zero());
            })
            .set(MISSION_RESET_BUTTON, ui);

        // Time delay
        Label::new("Time Delay: 0s")
            .xy((-ui.win_w / 2.0) + 70.0, (ui.win_h / 2.0) - 150.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(TIME_DELAY, ui);

        ////////////////////////////////////////////////////////////////////////////////////////////
        // Power section

        Label::new("Power")
            .xy((-ui.win_w / 2.0) + 110.0, (ui.win_h / 2.0) - 190.0)
            .font_size(20)
            .color(self.bg_color.plain_contrast())
            .set(POWER_LABEL, ui);

        // 48 bus

        Label::new(format!("48 Bus").as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 220.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(H_48_LABEL, ui);

        Label::new("NO DATA")
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 240.0)
            .font_size(16)
            .color(rgb(1.0, 0.0, 0.0))
            .set(H_48_V_VALUE, ui);

        Label::new("NO DATA")
            .xy((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 240.0)
            .font_size(16)
            .color(rgb(1.0, 0.0, 0.0))
            .set(H_48_A_VALUE, ui);

        // 24 bus

        Label::new(format!("24 H-Bus").as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 280.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(H_24_LABEL, ui);

        //casey added these here .. not sure if they reset or not >_> .. are these global? idk! yayyy
        // let h_24_avg_v_counter = 0.0;
        // let avg_v = 0.0;

        let (h_24_v, h_24_a, v_h_24_color) =
            match self.v_h_24 {
                    //  //Casey change here to try average out voltage stuff?
                    //  NEVERMIND THIS IS THE VERY WRONG WRONG PLACE
                    //  let avg = [0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0];
                    //  avg[i] = v;
                    //  //not sure if this counts 10 or 9
                    //  for x in 0..9{
                    //      let avg_v = avg_v + avg[x];
                    //  }
                    //  v = avg_v / 10;
                    //  h_24_avg_v_counter++;
                    //  if i > 9{
                    //      i = 0;
                    //  }
                     Some((v, a)) => {
                         (format!("{0:.2}V", v),
                          format!("{0:.2}A", a),
                          rgb(0.0, 1.0, 0.0))
                },
                None => {
                    ("NO DATA".to_string(),
                     "NO DATA".to_string(),
                     rgb(1.0, 0.0, 0.0))
                },
            };
        Label::new(h_24_v.as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 300.0)
            .font_size(16)
            .color(v_h_24_color)
            .set(H_24_V_VALUE, ui);

        Label::new(h_24_a.as_str())
            .xy((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 300.0)
            .font_size(16)
            .color(v_h_24_color)
            .set(H_24_A_VALUE, ui);

        // P-12 E bus

        Label::new(format!("P-12 E Bus").as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 340.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(P_12_E_LABEL, ui);

        let (volts_12, amps_12, v_p_12_e_color) =
            match self.v_p_12_e {
                Some((v, a)) => {
                    (format!("{0:.2}V", v),
                     format!("{0:.2}A", a),
                     rgb(0.0, 1.0, 0.0))
                },
                None => {
                    ("NO DATA".to_string(),
                     "NO DATA".to_string(),
                     rgb(1.0, 0.0, 0.0))
                },
            };
        Label::new(volts_12.as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 360.0)
            .font_size(16)
            .color(v_p_12_e_color)
            .set(P_12_E_V_VALUE, ui);

        Label::new(amps_12.as_str())
            .xy((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 360.0)
            .font_size(16)
            .color(v_p_12_e_color)
            .set(P_12_E_A_VALUE, ui);

        // P-12 PL bus

        Label::new(format!("P-12 PL Bus").as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 400.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(P_12_PL_LABEL, ui);

        let (p_12_pl_v, p_12_pl_a, v_p_12_pl_color) =
            match self.v_p_12_pl {
                Some((v, a)) => {
                    (format!("{0:.2}V", v),
                     format!("{0:.2}A", a),
                     rgb(0.0, 1.0, 0.0))
                },
                None => {
                    ("NO DATA".to_string(),
                     "NO DATA".to_string(),
                     rgb(1.0, 0.0, 0.0))
                },
            };
        Label::new(p_12_pl_v.as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 420.0)
            .font_size(16)
            .color(v_p_12_pl_color)
            .set(P_12_PL_V_VALUE, ui);

        Label::new(p_12_pl_a.as_str())
            .xy((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 420.0)
            .font_size(16)
            .color(v_p_12_pl_color)
            .set(P_12_PL_A_VALUE, ui);

        // Left motor

        Label::new(format!("L Motor").as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 460.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(L_MOTOR_POWER_LABEL, ui);

        Label::new("NO DATA")
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 480.0)
            .font_size(16)
            .color(rgb(1.0, 0.0, 0.0))
            .set(L_MOTOR_RPM_LABEL, ui);

        Label::new("NO DATA")
            .xy((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 480.0)
            .font_size(16)
            .color(rgb(1.0, 0.0, 0.0))
            .set(L_MOTOR_AMP_LABEL, ui);

        // Right motor

        Label::new(format!("R Motor").as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 520.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(R_MOTOR_POWER_LABEL, ui);

        Label::new("NO DATA")
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 540.0)
            .font_size(16)
            .color(rgb(1.0, 0.0, 0.0))
            .set(R_MOTOR_RPM_LABEL, ui);

        Label::new("NO DATA")
            .xy((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 540.0)
            .font_size(16)
            .color(rgb(1.0, 0.0, 0.0))
            .set(R_MOTOR_AMP_LABEL, ui);

        ////////////////////////////////////////////////////////////////////////////////////////////
        // Temp section

        Label::new("Temp")
            .xy((-ui.win_w / 2.0) + 410.0, (ui.win_h / 2.0) - 190.0)
            .font_size(20)
            .color(self.bg_color.plain_contrast())
            .set(TEMP_LABEL, ui);

        // Left motor temp

        Label::new(format!("L Motor").as_str())
            .xy((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 220.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(L_MOTOR_TEMP_LABEL, ui);

        let (l_motor_temp, l_motor_temp_color) =
            match self.l_motor_temp {
                Some(temp) => {
                    (format!("{0:.2} C", temp), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0)),
            };
        Label::new(l_motor_temp.as_str())
            .xy((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 220.0)
            .font_size(16)
            .color(l_motor_temp_color)
            .set(L_MOTOR_C_LABEL, ui);

        // Right motor temp

        Label::new(format!("R Motor").as_str())
            .xy((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 240.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(R_MOTOR_TEMP_LABEL, ui);

        let (r_motor_temp, r_motor_temp_color) =
            match self.r_motor_temp {
                Some(temp) => {
                    (format!("{0:.2} C", temp), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0)),
            };
        Label::new(r_motor_temp.as_str())
            .xy((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 240.0)
            .font_size(16)
            .color(r_motor_temp_color)
            .set(R_MOTOR_C_LABEL, ui);

        // Upper avionics box temp

        Label::new(format!("Upper Avionics").as_str())
            .xy((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 260.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(UPR_A_TEMP_LABEL, ui);

        let (upper_avionics_temp, upper_avionics_temp_color) =
            match self.upper_avionics_temp {
                Some(temp) => {
                    (format!("{0:.2} C", temp), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0)),
            };
        Label::new(upper_avionics_temp.as_str())
            .xy((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 260.0)
            .font_size(16)
            .color(upper_avionics_temp_color)
            .set(UPR_A_TEMP_VALUE, ui);

        // Lower avionics box temp

        Label::new(format!("Lower Avionics").as_str())
            .xy((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 280.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(LWR_A_TEMP_LABEL, ui);

        let (lower_avionics_temp, lower_avionics_temp_color) =
            match self.lower_avionics_temp {
                Some(temp) => {
                    (format!("{0:.2} C", temp), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0)),
            };
        Label::new(lower_avionics_temp.as_str())
            .xy((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 280.0)
            .font_size(16)
            .color(lower_avionics_temp_color)
            .set(LWR_A_TEMP_VALUE, ui);

        ////////////////////////////////////////////////////////////////////////////////////////////
        // Weather section

        Label::new("Weather")
            .xy((-ui.win_w / 2.0) + 410.0, (ui.win_h / 2.0) - 350.0)
            .font_size(20)
            .color(self.bg_color.plain_contrast())
            .set(WEATHER_LABEL, ui);

        // Wind speed

        Label::new(format!("Wind Speed").as_str())
            .xy((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 380.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(WIND_LABEL, ui);

        let (wind_speed, wind_speed_color) =
            match self.wind_speed {
                Some(wind_speed) => {
                    (format!("{0:.2} m/s", wind_speed), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0)),
            };
        Label::new(wind_speed.as_str())
            .xy((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 380.0)
            .font_size(16)
            .color(wind_speed_color)
            .set(WIND_VALUE, ui);

        // Altitude

        Label::new(format!("Altitude").as_str())
            .xy((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 400.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(ALTITUDE_LABEL, ui);

        let (altitude, altitude_color) =
            match self.altitude {
                Some(alt) => {
                    (format!("{0:.2} ft", alt), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0)),
            };
        Label::new(altitude.as_str())
            .xy((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 400.0)
            .font_size(16)
            .color(altitude_color)
            .set(ALTITUDE_VALUE, ui);

        // Pressure

        Label::new(format!("Pressure").as_str())
            .xy((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 420.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(PRESSURE_LABEL, ui);

        let (pressure, pressure_color) =
            match self.pressure {
                Some(pressure) => {
                    (format!("{0:.2} hPa", pressure), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0)),
            };
        Label::new(pressure.as_str())
            .xy((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 420.0)
            .font_size(16)
            .color(pressure_color)
            .set(PRESSURE_VALUE, ui);

        // Temp

        Label::new(format!("Temp").as_str())
            .xy((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 440.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(WEATHER_TEMP_LABEL, ui);

        let (temp, temp_color) =
            match self.temp {
                Some(temp) => {
                    (format!("{0:.2} C", temp), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0)),
            };
        Label::new(temp.as_str())
            .xy((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 440.0)
            .font_size(16)
            .color(temp_color)
            .set(WEATHER_TEMP_VALUE, ui);

        ////////////////////////////////////////////////////////////////////////////////////////////
        // IMU section

        Label::new("IMU")
            .xy((-ui.win_w / 2.0) + 410.0, (ui.win_h / 2.0) - 500.0)
            .font_size(20)
            .color(self.bg_color.plain_contrast())
            .set(IMU_LABEL, ui);

        let (pitch, roll, heading, color) =
            match self.pitch_roll_heading {
                Some((pitch, roll, heading)) => (format!("{0:.1}", pitch),
                                                 format!("{0:.1}", roll),
                                                 format!("{0:.1}", heading),
                                                 rgb(0.0, 1.0, 0.0)),
                None => ("NO DATA".to_string(), "NO DATA".to_string(),
                         "NO DATA".to_string(), rgb(1.0, 0.0, 0.0)),
            };

        // IMU pitch

        Label::new(format!("Pitch").as_str())
            .xy((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 530.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(IMU_PITCH_LABEL, ui);

        Label::new(pitch.as_str())
            .xy((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 530.0)
            .font_size(16)
            .color(l_motor_temp_color)
            .set(IMU_PITCH_VALUE, ui);

        // IMU roll

        Label::new(format!("Roll").as_str())
            .xy((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 550.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(IMU_ROLL_LABEL, ui);

        Label::new(roll.as_str())
            .xy((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 550.0)
            .font_size(16)
            .color(r_motor_temp_color)
            .set(IMU_ROLL_VALUE, ui);

        // IMU heading

        Label::new(format!("Heading").as_str())
            .xy((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 570.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(IMU_HEADING_LABEL, ui);

        Label::new(heading.as_str())
            .xy((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 570.0)
            .font_size(16)
            .color(r_motor_temp_color)
            .set(IMU_HEADING_VALUE, ui);

        // Draw our UI!
        ui.draw(c, gl);

        self.v12_graph.draw(c.trans(ui.win_w - 405.0, 100.0), gl, &mut *ui.glyph_cache.borrow_mut());
        self.motor_temp_graph.draw(c.trans(ui.win_w - 405.0, 320.0), gl, &mut *ui.glyph_cache.borrow_mut());
    }

    //woooo global counter variables because I can't let them reset and I don't know how to do that help me teddy you're my only hope
    let 24_h_avg_v_counter = 0;
    let 12_e_avg_v_counter = 0;
    let 12_pl_avg_v_counter = 0;

    //helloooooo casey just make a function instead of copying pasting -__-
    //but then again .. this is rust and I have no idea what I'm doing
    pub fn take_average(counter: i32){
        let

    }

    //functon to calibrate the IMU
    pub fun IMU_cal(){

    }

    pub fn handle_packet(&mut self, packet: String) {
        //println!("Got packet: {}", packet);
        let packet_parts: Vec<String> = packet.split(":").map(|s| s.to_string()).collect();

        match packet_parts[0].as_str() {
            "RPM_STATUS" => {
                self.l_rpm_status = packet_parts[1].clone();
                self.r_rpm_status = packet_parts[2].clone();
            },
            "VOLT" => {

                let point_x = self.v24_graph.num_points() as f64;
                let v_h_24 = packet_parts[1].parse().unwrap();
                //this is probably the place for Casey to put averaging things
                //I did put things in here! - Casey
                 let 24_temp = [0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0];
                 24_temp[24_h_avg_v_counter] = v_h_24;
                 //not sure if this sums or not .. yay internet
                 let avg_24_v = 24_temp.iter().fold(0, |avg_24_v, &b| a + b);
                 if(24_temp[9] != 0)
                 {
                     v_h_24 = avg_24_v / 10;
                 }
                 24_h_avg_v_counter++;
                 if 24_h_avg_v_counter > 9{
                     24_h_avg_v_counter = 0;
                 }

                self.v24_graph.add_point(point_x, v_h_24);
                if self.v24_graph.num_points() > 100 {
                    self.v24_graph.x_interval = ((self.v24_graph.num_points() - 100) as f64,
                                                  self.v24_graph.num_points() as f64);
                }
                self.v_h_24 = Some((v_h_24, 0.0));

                let point_x = self.v12_graph.num_points() as f64;
                let v_p_12_e = packet_parts[2].parse().unwrap();

                //I did put things in here! - Casey
                 let 12_e_temp = [0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0];
                 12_e_temp[12_e_avg_v_counter] = v_p_12_e;
                 //not sure if this counts 10 or 9
                 for x in 0..9{
                     let avg_12_v_e = avg_12_v_e + 12_e_temp[x];
                 }
                 //don't take the reading if we don't have 10 data points already lololloolol
                 if(12_e_temp[9] != 0)
                 {
                     v_p_12_e = avg_12_v_e / 10;
                 }
                 12_e_avg_v_counter++;
                 if 12_e_avg_v_counter > 9{
                     12_e_avg_v_counter = 0;
                 }

                self.v12_graph.add_point(point_x, v_p_12_e);
                if self.v12_graph.num_points() > 100 {
                    self.v12_graph.x_interval = ((self.v12_graph.num_points() - 100) as f64,
                                                  self.v12_graph.num_points() as f64);
                }
                self.v_p_12_e = Some((v_p_12_e, 0.0));

                let v_p_12_pl = packet_parts[3].parse().unwrap();

                //I did put things in here! - Casey
                 let 12_pl_temp = [0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0];
                 12_pl_temp[12_e_avg_v_counter] = v_p_12_pl;
                 //not sure if this counts 10 or 9
                 for x in 0..9{
                     let avg_12_v_pl = avg_12_v_pl + 12_pl_temp[x];
                 }
                 //don't take the reading if we don't have 10 data points already lololloolol
                 if(12_pl_temp[9] != 0)
                 {
                     v_p_12_pl = avg_12_v_pl / 10;
                 }
                 12_pl_avg_v_counter++;
                 if 12_pl_avg_v_counter > 9{
                     12_pl_avg_v_counter = 0;
                 }

                self.v_p_12_pl = Some((v_p_12_pl, 0.0));
            },
            "L_MOTOR_TEMP" => {
                let point_x = self.motor_temp_graph.num_points() as f64;
                let l_motor_temp = packet_parts[1].parse().unwrap();
                self.motor_temp_graph.add_point(point_x, l_motor_temp);
                if self.motor_temp_graph.num_points() > 100 {
                    self.motor_temp_graph.x_interval = ((self.motor_temp_graph.num_points() - 100) as f64,
                                                      self.motor_temp_graph.num_points() as f64);
                }
                self.l_motor_temp = Some(l_motor_temp);
            },
            "R_MOTOR_TEMP" => {
                //let point_x = self.motor_temp_graph.num_points() as f64;
                let r_motor_temp = packet_parts[1].parse().unwrap();
                /*self.motor_temp_graph.add_point(point_x, l_motor_temp);
                if self.motor_temp_graph.num_points() > 100 {
                    self.motor_temp_graph.x_interval = ((self.motor_temp_graph.num_points() - 100) as f64,
                                                      self.motor_temp_graph.num_points() as f64);
                }*/
                self.r_motor_temp = Some(r_motor_temp);
            },
            "UPR_A_TEMP" => {
                let temp = packet_parts[1].parse().unwrap();
                self.upper_avionics_temp = Some(temp);
            },
            "LWR_A_TEMP" => {
                let temp = packet_parts[1].parse().unwrap();
                self.lower_avionics_temp = Some(temp);
            },
            "W_TEMP" => {
                let temp = packet_parts[1].parse().unwrap();
                self.temp = Some(temp);
            },
            "W_PR_ALT" => {
                let pressure = packet_parts[1].parse().unwrap();
                let altitude= packet_parts[2].parse().unwrap();
                self.pressure = Some(pressure);
                self.altitude = Some(altitude);
            },
            "W_WND_SPD" => {
                let wind_speed = packet_parts[1].parse().unwrap();
                self.wind_speed = Some(wind_speed);
            },
            //Casey adding L+R motor currents
            "L_MOTOR_CURRENT" => {
                let left_motor_current = packet_parts[1].parse().unwrap();
                self.left_motor_current  = Some(left_motor_current);
            },
            "R_MOTOR_CURRENT" => {
                let right_motor_current = packet_parts[1].parse().unwrap();
                self.right_motor_current  = Some(right_motor_current);
            },
            "IMU" => {
                let ax: f64 = packet_parts[1].parse().unwrap();
                let ay: f64 = packet_parts[2].parse().unwrap();
                let az: f64 = packet_parts[3].parse().unwrap();

                let mx: f64 = packet_parts[7].parse().unwrap();
                let my: f64 = packet_parts[8].parse().unwrap();
                let mz: f64 = packet_parts[9].parse().unwrap();

                let roll = f64::atan2(ay, az);
                let pitch = f64::atan2(-ax, ay*f64::sin(roll) + az*f64::cos(roll));
                let heading = f64::atan2(mz*f64::sin(roll) - my*f64::cos(roll),
                                         mx*f64::cos(pitch) + my*f64::sin(pitch)*f64::sin(roll) + mz*f64::sin(pitch)*f64::cos(roll));

                self.pitch_roll_heading = Some((pitch.to_degrees(), roll.to_degrees(), heading.to_degrees()));
            },
            _ => { println!("WARNING: Unknown packet ID: {}", packet_parts[0]) },
        }
    }

    pub fn on_key_pressed(&mut self, key: input::Key) {
        match key {
            _ => { },
        }
    }

    pub fn on_key_released(&mut self, key: input::Key) {
        match key {
            _ => { },
        }
    }
}

// Widget IDs
const LOCAL_TIME: WidgetId = 0;
const UTC_TIME: WidgetId = LOCAL_TIME + 1;
const MISSION_TIME_LABEL: WidgetId = UTC_TIME + 1;
const MISSION_START_BUTTON: WidgetId = MISSION_TIME_LABEL + 1;
const MISSION_RESET_BUTTON: WidgetId = MISSION_START_BUTTON + 1;
const TIME_DELAY: WidgetId = MISSION_RESET_BUTTON + 1;

// Power section
const POWER_LABEL: WidgetId = TIME_DELAY + 1;

const H_48_LABEL: WidgetId = POWER_LABEL + 1;
const H_48_V_VALUE: WidgetId = H_48_LABEL + 1;
const H_48_A_VALUE: WidgetId = H_48_V_VALUE + 1;

const H_24_LABEL: WidgetId = H_48_A_VALUE + 1;
const H_24_V_VALUE: WidgetId = H_24_LABEL + 1;
const H_24_A_VALUE: WidgetId = H_24_V_VALUE + 1;

const P_12_E_LABEL: WidgetId = H_24_A_VALUE + 1;
const P_12_E_V_VALUE: WidgetId = P_12_E_LABEL + 1;
const P_12_E_A_VALUE: WidgetId = P_12_E_V_VALUE + 1;

const P_12_PL_LABEL: WidgetId = P_12_E_A_VALUE + 1;
const P_12_PL_V_VALUE: WidgetId = P_12_PL_LABEL + 1;
const P_12_PL_A_VALUE: WidgetId = P_12_PL_V_VALUE + 1;

const L_MOTOR_POWER_LABEL: WidgetId = P_12_PL_A_VALUE + 1;
const L_MOTOR_RPM_LABEL: WidgetId = L_MOTOR_POWER_LABEL + 1;
const L_MOTOR_AMP_LABEL: WidgetId = L_MOTOR_RPM_LABEL + 1;

const R_MOTOR_POWER_LABEL: WidgetId = L_MOTOR_AMP_LABEL + 1;
const R_MOTOR_RPM_LABEL: WidgetId = R_MOTOR_POWER_LABEL + 1;
const R_MOTOR_AMP_LABEL: WidgetId = R_MOTOR_RPM_LABEL + 1;

// Temp section
const TEMP_LABEL: WidgetId = R_MOTOR_AMP_LABEL + 1;

const L_MOTOR_TEMP_LABEL: WidgetId = TEMP_LABEL + 1;
const L_MOTOR_C_LABEL: WidgetId = L_MOTOR_TEMP_LABEL + 1;

const R_MOTOR_TEMP_LABEL: WidgetId = L_MOTOR_C_LABEL + 1;
const R_MOTOR_C_LABEL: WidgetId = R_MOTOR_TEMP_LABEL + 1;

const UPR_A_TEMP_LABEL: WidgetId = R_MOTOR_C_LABEL + 1;
const UPR_A_TEMP_VALUE: WidgetId = UPR_A_TEMP_LABEL + 1;

const LWR_A_TEMP_LABEL: WidgetId = UPR_A_TEMP_VALUE + 1;
const LWR_A_TEMP_VALUE: WidgetId = LWR_A_TEMP_LABEL + 1;

// Weather section
const WEATHER_LABEL: WidgetId = LWR_A_TEMP_VALUE + 1;

const WIND_LABEL: WidgetId = WEATHER_LABEL + 1;
const WIND_VALUE: WidgetId = WIND_LABEL + 1;

const ALTITUDE_LABEL: WidgetId = WIND_VALUE + 1;
const ALTITUDE_VALUE: WidgetId = ALTITUDE_LABEL + 1;

const PRESSURE_LABEL: WidgetId = ALTITUDE_VALUE + 1;
const PRESSURE_VALUE: WidgetId = PRESSURE_LABEL + 1;

const WEATHER_TEMP_LABEL: WidgetId = PRESSURE_VALUE + 1;
const WEATHER_TEMP_VALUE: WidgetId = WEATHER_TEMP_LABEL + 1;

// IMU section
const IMU_LABEL: WidgetId = WEATHER_TEMP_VALUE + 1;

const IMU_PITCH_LABEL: WidgetId = IMU_LABEL + 1;
const IMU_PITCH_VALUE: WidgetId = IMU_PITCH_LABEL + 1;

const IMU_ROLL_LABEL: WidgetId = IMU_PITCH_VALUE + 1;
const IMU_ROLL_VALUE: WidgetId = IMU_ROLL_LABEL + 1;

const IMU_HEADING_LABEL: WidgetId = IMU_ROLL_VALUE + 1;
const IMU_HEADING_VALUE: WidgetId = IMU_HEADING_LABEL + 1;
