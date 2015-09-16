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

use avg_val::AvgVal;
use line_graph::LineGraph;

enum MissionTime {
    Paused(time::Duration),
    Running(time::Tm, time::Duration),
}

pub enum RygLimit {
    LessThan(f64, f64),
    GreaterThan(f64, f64),
}

impl RygLimit {
    pub fn get_color(&self, value: f64) -> Color {
        match *self {
            RygLimit::LessThan(r, y) => {
                if value < r {
                    rgb(1.0, 0.0, 0.0)
                } else if value < y {
                    rgb(1.0, 1.0, 0.0)
                } else {
                    rgb(0.0, 1.0, 0.0)
                }
            },
            RygLimit::GreaterThan(r, y) => {
                if value > r {
                    rgb(1.0, 0.0, 0.0)
                } else if value > y {
                    rgb(1.0, 1.0, 0.0)
                } else {
                    rgb(0.0, 1.0, 0.0)
                }
            },
        }
    }
}

pub struct TelemetryUi {
    socket: UdpSocket,

    bg_color: Color,

    mission_time: MissionTime,

    // RPM stuff
    l_rpm_status: String,
    r_rpm_status: String,

    // Voltage stuff
    v48_graph: LineGraph,
    h_48_v: AvgVal,
    h_48_v_limits: RygLimit,

    a24_graph: LineGraph,
    h_24_v: AvgVal,
    h_24_a: AvgVal,
    h_24_v_limits: RygLimit,

    v12_graph: LineGraph,
    p_12_e_v: AvgVal,
    p_12_e_a: AvgVal,
    p_12_e_v_limits: RygLimit,

    p_12_pl_v: AvgVal,
    p_12_pl_v_limits: RygLimit,

    l_motor_amp: AvgVal,
    r_motor_amp: AvgVal,

    // GPS
    latitude: Option<f64>,
    longitude: Option<f64>,
    speed: Option<f64>,
    gps_altitude: Option<f64>,
    angle: Option<f64>,

    // Motor temp
    motor_temp_graph: LineGraph,
    l_motor_temp: AvgVal,
    r_motor_temp: AvgVal,
    l_motor_temp_limits: RygLimit,
    r_motor_temp_limits: RygLimit,

    // Avionics box temp
    upper_avionics_temp: AvgVal,
    lower_avionics_temp: AvgVal,
    upper_avionics_temp_limits: RygLimit,
    lower_avionics_temp_limits: RygLimit,

    // Weather section
    wind_speed: AvgVal,
    pressure: Option<f64>,
    altitude: Option<f64>,
    temp: Option<f64>,

    // IMU
    pitch_roll_heading: Option<(f64, f64, f64)>,
}

impl TelemetryUi {
    pub fn new(socket: UdpSocket) -> TelemetryUi {
        let v48_graph = LineGraph::new((400.0, 150.0), (0.0, 4.0 * 3600.0 * 2.0), (0.0, 80.0), vec![[1.0, 0.0, 0.0, 1.0]]);
        let a24_graph = LineGraph::new((400.0, 150.0), (0.0, 4.0 * 3600.0 * 2.0), (0.0, 40.0), vec![[1.0, 0.0, 0.0, 1.0]]);
        let v12_graph = LineGraph::new((400.0, 150.0), (0.0, 4.0 * 3600.0 * 2.0), (0.0, 20.0), vec![[1.0, 0.0, 0.0, 1.0]]);
        let motor_temp_graph = LineGraph::new((400.0, 150.0),
                                              (0.0, 4.0 * 3600.0 * 2.0),
                                              (0.0, 100.0),
                                              vec![[1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0]]);

        TelemetryUi {
            socket: socket,

            bg_color: rgb(0.2, 0.35, 0.45),

            mission_time: MissionTime::Paused(time::Duration::zero()),

            l_rpm_status: "NO DATA".to_string(),
            r_rpm_status: "NO DATA".to_string(),

            v48_graph: v48_graph,
            h_48_v: AvgVal::new(60),
            h_48_v_limits: RygLimit::LessThan(45.0, 48.0),

            a24_graph: a24_graph,
            h_24_v: AvgVal::new(60),
            h_24_a: AvgVal::new(30),
            h_24_v_limits: RygLimit::LessThan(22.0, 24.0),

            v12_graph: v12_graph,
            p_12_e_v: AvgVal::new(60),
            p_12_e_a: AvgVal::new(30),
            p_12_e_v_limits: RygLimit::LessThan(10.0, 12.0),

            p_12_pl_v: AvgVal::new(60),
            p_12_pl_v_limits: RygLimit::LessThan(10.0, 12.0),

            l_motor_amp: AvgVal::new(30),
            r_motor_amp: AvgVal::new(30),

            // GPS
            latitude: None,
            longitude: None,
            speed: None,
            gps_altitude: None,
            angle: None,

            motor_temp_graph: motor_temp_graph,
            l_motor_temp: AvgVal::new(40),
            r_motor_temp: AvgVal::new(40),
            l_motor_temp_limits: RygLimit::GreaterThan(80.0, 60.0),
            r_motor_temp_limits: RygLimit::GreaterThan(80.0, 60.0),

            upper_avionics_temp: AvgVal::new(60),
            lower_avionics_temp: AvgVal::new(60),
            upper_avionics_temp_limits: RygLimit::GreaterThan(60.0, 45.0),
            lower_avionics_temp_limits: RygLimit::GreaterThan(60.0, 45.0),

            wind_speed: AvgVal::new(20),
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

        let (h_48_v, h_48_v_color) =
            match self.h_48_v.get() {
                Some(v) => {
                    (format!("{0:.2}V", v), self.h_48_v_limits.get_color(v))
                },
                None => {
                    ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0))
                },
            };
        Label::new(h_48_v.as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 240.0)
            .font_size(16)
            .color(h_48_v_color)
            .set(H_48_V_VALUE, ui);

        /*Label::new("NO DATA")
            .xy((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 240.0)
            .font_size(16)
            .color(rgb(1.0, 0.0, 0.0))
            .set(H_48_A_VALUE, ui);*/

        // 24 bus

        Label::new(format!("24 H-Bus").as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 280.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(H_24_LABEL, ui);

        let (h_24_v, h_24_v_color) =
            match self.h_24_v.get() {
                Some(v) => {
                    (format!("{0:.2}V", v), self.h_24_v_limits.get_color(v))
                },
                None => {
                    ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0))
                },
            };
        let (h_24_a, h_24_a_color) =
            match self.h_24_a.get() {
                Some(a) => {
                    (format!("{0:.2}A", a), rgb(0.0, 1.0, 0.0))
                },
                None => {
                    ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0))
                },
            };
        Label::new(h_24_v.as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 300.0)
            .font_size(16)
            .color(h_24_v_color)
            .set(H_24_V_VALUE, ui);

        Label::new(h_24_a.as_str())
            .xy((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 300.0)
            .font_size(16)
            .color(h_24_a_color)
            .set(H_24_A_VALUE, ui);

        // P-12 E bus

        Label::new(format!("P-12 E Bus").as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 340.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(P_12_E_LABEL, ui);

        let (p_12_e_v, p_12_e_v_color) =
            match self.p_12_e_v.get() {
                Some(v) => {
                    (format!("{0:.2}V", v), self.p_12_e_v_limits.get_color(v))
                },
                None => {
                    ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0))
                },
            };
        let (p_12_e_a, p_12_e_a_color) =
            match self.p_12_e_a.get() {
                Some(a) => {
                    (format!("{0:.2}A", a), rgb(0.0, 1.0, 0.0))
                },
                None => {
                    ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0))
                },
            };
        Label::new(p_12_e_v.as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 360.0)
            .font_size(16)
            .color(p_12_e_v_color)
            .set(P_12_E_V_VALUE, ui);

        Label::new(p_12_e_a.as_str())
            .xy((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 360.0)
            .font_size(16)
            .color(p_12_e_a_color)
            .set(P_12_E_A_VALUE, ui);

        // P-12 PL bus

        Label::new(format!("P-12 PL Bus").as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 400.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(P_12_PL_LABEL, ui);

        let (p_12_pl_v, p_12_pl_v_color) =
            match self.p_12_pl_v.get() {
                Some(v) => {
                    (format!("{0:.2}V", v), self.p_12_pl_v_limits.get_color(v))
                },
                None => {
                    ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0))
                },
            };
        Label::new(p_12_pl_v.as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 420.0)
            .font_size(16)
            .color(p_12_pl_v_color)
            .set(P_12_PL_V_VALUE, ui);

        /*Label::new(p_12_pl_a.as_str())
            .xy((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 420.0)
            .font_size(16)
            .color(p_12_pl_v_color)
            .set(P_12_PL_A_VALUE, ui);*/

        // Left motor

        Label::new(format!("L Motor").as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 460.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(L_MOTOR_POWER_LABEL, ui);

        let (l_motor_amp, l_motor_amp_color) =
            match self.l_motor_amp.get() {
                Some(amp) => {
                    (format!("{0:.2}A", amp), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0)),
            };
        Label::new(l_motor_amp.as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 480.0)
            .font_size(16)
            .color(l_motor_amp_color)
            .set(L_MOTOR_AMP_LABEL, ui);

        // Right motor

        Label::new(format!("R Motor").as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 520.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(R_MOTOR_POWER_LABEL, ui);

        let (r_motor_amp, r_motor_amp_color) =
            match self.r_motor_amp.get() {
                Some(amp) => {
                    (format!("{0:.2}A", amp), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0)),
            };
        Label::new(r_motor_amp.as_str())
            .xy((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 540.0)
            .font_size(16)
            .color(l_motor_amp_color)
            .set(R_MOTOR_AMP_LABEL, ui);

        ////////////////////////////////////////////////////////////////////////////////////////////
        // GPS section

        Label::new("GPS")
            .xy((-ui.win_w / 2.0) + 410.0, (ui.win_h / 2.0) - 50.0)
            .font_size(22)
            .color(self.bg_color.plain_contrast())
            .set(GPS_LABEL, ui);
        
        // Latitude label
        let (latitude, latitude_color) =
            match self.latitude {
                Some(lat) => {
                    (format!("{0:.2} N", lat), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0)),
            };
        Label::new(latitude.as_str())
            .xy((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 75.0)
            .font_size(16)
            .color(latitude_color)
            .set(LATITUDE_LABEL, ui);

        // Longitude label
        let (longitude, longitude_color) =
            match self.longitude {
                Some(lng) => {
                    (format!("{0:.2} W", lng), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0)),
            };
        Label::new(longitude.as_str())
            .xy((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 95.0)
            .font_size(16)
            .color(longitude_color)
            .set(LONGITUDE_LABEL, ui);
        
        // Speed label
        let (speed, speed_color) =
            match self.speed {
                Some(speed) => {
                    (format!("{0:.2} m/s", speed), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0)),
            };
        Label::new(speed.as_str())
            .xy((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 115.0)
            .font_size(16)
            .color(speed_color)
            .set(SPEED_LABEL, ui);

        // Altitude label
        let (gps_altitude, gps_altitude_color) =
            match self.gps_altitude {
                Some(alt) => {
                    (format!("{0:.2} m", alt), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0)),
            };
        Label::new(gps_altitude.as_str())
            .xy((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 135.0)
            .font_size(16)
            .color(gps_altitude_color)
            .set(GPS_ALTITUDE_LABEL, ui);

        // Angle label
        let (angle, angle_color) =
            match self.angle {
                Some(angle) => {
                    (format!("{0:.2} deg", angle), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(1.0, 0.0, 0.0)),
            };
        Label::new(angle.as_str())
            .xy((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 155.0)
            .font_size(16)
            .color(angle_color)
            .set(ANGLE_LABEL, ui);

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
            match self.l_motor_temp.get() {
                Some(temp) => {
                    (format!("{0:.2} C", temp), self.l_motor_temp_limits.get_color(temp))
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
            match self.r_motor_temp.get() {
                Some(temp) => {
                    (format!("{0:.2} C", temp), self.r_motor_temp_limits.get_color(temp))
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
            match self.upper_avionics_temp.get() {
                Some(temp) => {
                    (format!("{0:.2} C", temp), self.upper_avionics_temp_limits.get_color(temp))
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
            match self.lower_avionics_temp.get() {
                Some(temp) => {
                    (format!("{0:.2} C", temp), self.lower_avionics_temp_limits.get_color(temp))
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
            match self.wind_speed.get() {
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

        let (pitch, roll, heading, imu_color) =
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
            .color(imu_color)
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
            .color(imu_color)
            .set(IMU_ROLL_VALUE, ui);

        // IMU heading

        Label::new("Heading")
            .xy((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 570.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(IMU_HEADING_LABEL, ui);

        Label::new(heading.as_str())
            .xy((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 570.0)
            .font_size(16)
            .color(imu_color)
            .set(IMU_HEADING_VALUE, ui);

        // Trend graph labels
        Label::new("H-48 V")
            .xy((ui.win_w / 2.0) - 405.0 - 80.0, (ui.win_h / 2.0) - 90.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(TREND_H_48V_LABEL, ui);
        Label::new("H-24 A")
            .xy((ui.win_w / 2.0) - 405.0 - 80.0, (ui.win_h / 2.0) - 270.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(TREND_H_24A_LABEL, ui);
        Label::new("P-12 E V")
            .xy((ui.win_w / 2.0) - 405.0 - 80.0, (ui.win_h / 2.0) - 450.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(TREND_P_12_E_V_LABEL, ui);
        Label::new("LR Motor Temp")
            .xy((ui.win_w / 2.0) - 405.0 - 80.0, (ui.win_h / 2.0) - 630.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(TREND_LR_MOTOR_TEMP_LABEL, ui);

        // Draw our UI!
        ui.draw(c, gl);

        self.v48_graph.draw(c.trans(ui.win_w - 405.0, 5.0), gl, &mut *ui.glyph_cache.borrow_mut());
        self.a24_graph.draw(c.trans(ui.win_w - 405.0, 185.0), gl, &mut *ui.glyph_cache.borrow_mut());
        self.v12_graph.draw(c.trans(ui.win_w - 405.0, 365.0), gl, &mut *ui.glyph_cache.borrow_mut());
        self.motor_temp_graph.draw(c.trans(ui.win_w - 405.0, 545.0), gl, &mut *ui.glyph_cache.borrow_mut());
    }

    pub fn handle_packet(&mut self, packet: String) {
        let packets = packet.split("|");
        
        for packet in packets {
            let packet_parts: Vec<String> = packet.split(":").map(|s| s.to_string()).collect();

            match packet_parts[0].as_str() {
                "RPM_STATUS" => {
                    self.l_rpm_status = packet_parts[1].clone();
                    self.r_rpm_status = packet_parts[2].clone();
                },
                "VOLT" => {
                    /////////////////////
                    self.h_48_v.add_value(packet_parts[1].parse().unwrap());
                    let h_48_v = self.h_48_v.get().unwrap();

                    let point_x = self.v48_graph.num_points(0) as f64;
                    self.v48_graph.add_point(0, point_x, h_48_v);

                    /////////////////////
                    self.h_24_v.add_value(packet_parts[2].parse().unwrap());

                    /////////////////////
                    self.p_12_e_v.add_value(packet_parts[3].parse().unwrap());
                    let p_12_e_v = self.p_12_e_v.get().unwrap();

                    let point_x = self.v12_graph.num_points(0) as f64;
                    self.v12_graph.add_point(0, point_x, p_12_e_v);

                    /////////////////////
                    self.p_12_pl_v.add_value(packet_parts[4].parse().unwrap());
                },
                "AMP" => {
                    self.l_motor_amp.add_value(packet_parts[1].parse().unwrap());
                    self.r_motor_amp.add_value(packet_parts[2].parse().unwrap());
                    self.p_12_e_a.add_value(packet_parts[3].parse().unwrap());
                    
                    // h-24
                    self.h_24_a.add_value(packet_parts[4].parse().unwrap());
                    let h_24_a = self.p_12_e_v.get().unwrap();

                    let point_x = self.a24_graph.num_points(0) as f64;
                    self.a24_graph.add_point(0, point_x, h_24_a);
                },
                "GPS" => {
                    if packet_parts.len() == 6 {
                        self.latitude = packet_parts[1].parse().ok();
                        self.longitude = packet_parts[2].parse().ok();
                        self.speed = packet_parts[3].parse().ok();
                        self.gps_altitude = packet_parts[4].parse().ok();
                        self.angle = packet_parts[5].parse().ok();
                    }
                },
                "L_MOTOR_TEMP" => {
                    self.l_motor_temp.add_value(packet_parts[1].parse().unwrap());
                    let l_motor_temp = self.l_motor_temp.get().unwrap();

                    let point_x = self.motor_temp_graph.num_points(0) as f64;
                    self.motor_temp_graph.add_point(0, point_x, l_motor_temp);
                },
                "R_MOTOR_TEMP" => {
                    self.r_motor_temp.add_value(packet_parts[1].parse().unwrap());
                    let r_motor_temp = self.r_motor_temp.get().unwrap();

                    let point_x = self.motor_temp_graph.num_points(1) as f64;
                    self.motor_temp_graph.add_point(1, point_x, r_motor_temp);
                },
                "UPR_A_TEMP" => {
                    self.upper_avionics_temp.add_value(packet_parts[1].parse().unwrap());
                },
                "LWR_A_TEMP" => {
                    self.lower_avionics_temp.add_value(packet_parts[1].parse().unwrap());
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
                    self.wind_speed.add_value(packet_parts[1].parse().unwrap());
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

                    let mut heading = heading.to_degrees();
                    if heading < 0.0 {
                        heading += 360.0;
                    }
                    heading = 360.0 - heading;
                    self.pitch_roll_heading = Some((pitch.to_degrees(), roll.to_degrees(), heading));
                },
                _ => { println!("WARNING: Unknown packet ID: {}", packet_parts[0]) },
            }
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

// GPS section
const GPS_LABEL: WidgetId = R_MOTOR_AMP_LABEL + 1;
const LATITUDE_LABEL: WidgetId = GPS_LABEL + 1;
const LONGITUDE_LABEL: WidgetId = LATITUDE_LABEL + 1;
const SPEED_LABEL: WidgetId = LONGITUDE_LABEL + 1;
const GPS_ALTITUDE_LABEL: WidgetId = SPEED_LABEL + 1;
const ANGLE_LABEL: WidgetId = GPS_ALTITUDE_LABEL + 1;

// Temp section
const TEMP_LABEL: WidgetId = ANGLE_LABEL + 1;

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

// Trend graph labels
const TREND_H_48V_LABEL: WidgetId = IMU_HEADING_VALUE + 1;
const TREND_H_24A_LABEL: WidgetId = TREND_H_48V_LABEL + 1;
const TREND_P_12_E_V_LABEL: WidgetId = TREND_H_24A_LABEL + 1;
const TREND_LR_MOTOR_TEMP_LABEL: WidgetId = TREND_P_12_E_V_LABEL + 1;
