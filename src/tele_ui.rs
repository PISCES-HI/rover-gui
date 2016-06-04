use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};

use conrod::{
    self,
    Background,
    Button,
    Color,
    Colorable,
    Frameable,
    Labelable,
    Positionable,
    Sizeable,
    Text,
    Widget,
};
use conrod::color::rgb;
use graphics::{Context, Graphics};
use piston_window;
use time;

use avg_val::AvgVal;
use conrod_config;
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
    bg_color: Color,

    mission_time: MissionTime,

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

    // Temperature sensors
    upper_avionics_temp: AvgVal,
    lower_avionics_temp: AvgVal,
    ambient_temp: AvgVal,
    upper_avionics_temp_limits: RygLimit,
    lower_avionics_temp_limits: RygLimit,

    // Weather section
    wind_speed: AvgVal,
    pressure: Option<f64>,
    altitude: Option<f64>,
    temp: Option<f64>,

    // IMU
    pitch_roll_heading: Option<(f64, f64, f64)>,

    log_files: HashMap<String, BufWriter<File>>,
}

impl TelemetryUi {
    pub fn new(mission_folder: &str) -> TelemetryUi {
        let v48_graph = LineGraph::new((400.0, 150.0), (0.0, 4.0 * 3600.0 * 2.0), (0.0, 80.0), vec![[1.0, 0.0, 0.0, 1.0]]);
        let a24_graph = LineGraph::new((400.0, 150.0), (0.0, 4.0 * 3600.0 * 2.0), (0.0, 40.0), vec![[1.0, 0.0, 0.0, 1.0]]);
        let v12_graph = LineGraph::new((400.0, 150.0), (0.0, 4.0 * 3600.0 * 2.0), (0.0, 20.0), vec![[1.0, 0.0, 0.0, 1.0]]);
        let motor_temp_graph = LineGraph::new((400.0, 150.0),
                                              (0.0, 4.0 * 3600.0 * 2.0),
                                              (0.0, 100.0),
                                              vec![[1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0]]);

        // Create the log files
        let mut log_files = HashMap::new();
        log_files.insert("imu".to_string(),
                         BufWriter::new(File::create(format!("mission_data/{}/imu",
                                                             mission_folder).as_str()).unwrap()));
        log_files.insert("gps".to_string(),
                         BufWriter::new(File::create(format!("mission_data/{}/gps",
                                                             mission_folder).as_str()).unwrap()));
        log_files.insert("volt".to_string(),
                         BufWriter::new(File::create(format!("mission_data/{}/volt",
                                                             mission_folder).as_str()).unwrap()));
        log_files.insert("amp".to_string(),
                         BufWriter::new(File::create(format!("mission_data/{}/amp",
                                                             mission_folder).as_str()).unwrap()));
        log_files.insert("temp".to_string(),
                         BufWriter::new(File::create(format!("mission_data/{}/motor_temp",
                                                             mission_folder).as_str()).unwrap()));
        log_files.insert("weather".to_string(),
                         BufWriter::new(File::create(format!("mission_data/{}/weather",
                                                             mission_folder).as_str()).unwrap()));
        // Write log headers
        log_files.get_mut("imu").unwrap().write_all("#pitch\troll\theading\n".as_bytes()).unwrap();
        log_files.get_mut("gps")
                 .unwrap()
                 .write_all("#latitude\tlongitude\tspeed\taltitude\tangle\n".as_bytes())
                 .unwrap();
        log_files.get_mut("volt")
                 .unwrap()
                 .write_all("#H-48v\tH-24v\tP-12v E\tP-12-v PL\n".as_bytes())
                 .unwrap();
        log_files.get_mut("amp")
                 .unwrap()
                 .write_all("#H-24v\tP-12v E\ttL motor\tR motor\n".as_bytes())
                 .unwrap();
        log_files.get_mut("temp")
                 .unwrap()
                 .write_all("#L motor\tR motor\tUpper Avionics\tLower Avionics\n".as_bytes())
                 .unwrap();
        log_files.get_mut("weather")
                 .unwrap()
                 .write_all("#wind speed\tpressure\taltitude\ttemp\n".as_bytes())
                 .unwrap();

        TelemetryUi {
            bg_color: rgb(0.2, 0.35, 0.45),

            mission_time: MissionTime::Paused(time::Duration::zero()),

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

            upper_avionics_temp: AvgVal::new(30),
            lower_avionics_temp: AvgVal::new(30),
            ambient_temp: AvgVal::new(30),
            upper_avionics_temp_limits: RygLimit::GreaterThan(60.0, 45.0),
            lower_avionics_temp_limits: RygLimit::GreaterThan(60.0, 45.0),

            wind_speed: AvgVal::new(20),
            pressure: None,
            altitude: None,
            temp: None,

            pitch_roll_heading: None,

            log_files: log_files,
        }
    }

    pub fn log_data(&mut self) {
        // imu
        match self.pitch_roll_heading {
            Some((pitch, roll, heading)) => {
                write!(&mut self.log_files.get_mut("imu").unwrap(),
                       "{}\t{}\t{}\n", pitch, roll, heading).unwrap();
            },
            None => { write!(&mut self.log_files.get_mut("imu").unwrap(), "none").unwrap(); },
        }
        // gps
        write!(&mut self.log_files.get_mut("gps").unwrap(),
               "{:?}\t{:?}\t{:?}\t{:?}\t{:?}\n", self.latitude, self.longitude,
               self.speed, self.gps_altitude, self.angle).unwrap();
        // volt
        write!(&mut self.log_files.get_mut("volt").unwrap(),
               "{:?}\t{:?}\t{:?}\t{:?}\n", self.h_48_v.get(), self.h_24_v.get(),
               self.p_12_e_v.get(), self.p_12_pl_v.get()).unwrap();
        // amp
        write!(&mut self.log_files.get_mut("amp").unwrap(),
               "{:?}\t{:?}\t{:?}\t{:?}\n", self.h_24_a.get(), self.p_12_e_a.get(),
               self.l_motor_amp.get(), self.r_motor_amp.get()).unwrap();
        // temp
        write!(&mut self.log_files.get_mut("temp").unwrap(),
               "{:?}\t{:?}\t{:?}\t{:?}\n", self.l_motor_temp.get(), self.r_motor_temp.get(),
               self.upper_avionics_temp.get(), self.lower_avionics_temp.get()).unwrap();
        // weather
        write!(&mut self.log_files.get_mut("weather").unwrap(),
               "{:?}\t{:?}\t{:?}\t{:?}\n", self.wind_speed.get(), self.pressure,
               self.altitude, self.temp).unwrap();
    }

    pub fn draw_ui<'a, G>(&mut self, c: Context, g: &mut G, ui: &mut conrod_config::Ui)
                          where G: Graphics<Texture=<piston_window::G2d<'static> as conrod::Graphics>::Texture> {
        use graphics::{Transformed};

        // Draw the background.
        Background::new().color(self.bg_color).set(ui);

        ui.set_widgets(|ref mut ui| {
            self.set_widgets(ui);
        });

        // Draw our UI!
        ui.draw(c, g);

        self.v48_graph.draw(c.trans(ui.win_w - 405.0, 5.0), g, &mut *ui.glyph_cache.borrow_mut());
        self.a24_graph.draw(c.trans(ui.win_w - 405.0, 185.0), g, &mut *ui.glyph_cache.borrow_mut());
        self.v12_graph.draw(c.trans(ui.win_w - 405.0, 365.0), g, &mut *ui.glyph_cache.borrow_mut());
        self.motor_temp_graph.draw(c.trans(ui.win_w - 405.0, 545.0), g, &mut *ui.glyph_cache.borrow_mut());
    }

    pub fn set_widgets(&mut self, ui: &mut conrod_config::UiCell) {
        let time_now = time::now();

        // Local time
        Text::new(format!("{}", time_now.strftime("Local  %x  %X").unwrap()).as_str())
            .x_y((-ui.win_w / 2.0) + 100.0, (ui.win_h / 2.0) - 10.0)
            .font_size(16)
            .color(self.bg_color.plain_contrast())
            .set(LOCAL_TIME, ui);

        // UTC time
        Text::new(format!("{}", time_now.to_utc().strftime("%Z  %x  %X").unwrap()).as_str())
            .x_y((-ui.win_w / 2.0) + 104.0, (ui.win_h / 2.0) - 30.0)
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
        Text::new(format!("Mission Time: {}:{}:{}:{}", days, hours, minutes, seconds).as_str())
            .x_y((-ui.win_w / 2.0) + 150.0, (ui.win_h / 2.0) - 70.0)
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
            .w_h(100.0, 30.0)
            .x_y((-ui.win_w / 2.0) + 55.0, (ui.win_h / 2.0) - 100.0)
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
            .w_h(100.0, 30.0)
            .x_y((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 100.0)
            .rgb(0.3, 0.8, 0.3)
            .frame(1.0)
            .label("Reset")
            .react(|| {
                self.mission_time = MissionTime::Paused(time::Duration::zero());
            })
            .set(MISSION_RESET_BUTTON, ui);

        // Time delay
        Text::new("Time Delay: 0s")
            .x_y((-ui.win_w / 2.0) + 70.0, (ui.win_h / 2.0) - 150.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(TIME_DELAY, ui);

        ////////////////////////////////////////////////////////////////////////////////////////////
        // Power section

        Text::new("Power")
            .x_y((-ui.win_w / 2.0) + 110.0, (ui.win_h / 2.0) - 190.0)
            .font_size(20)
            .color(self.bg_color.plain_contrast())
            .set(POWER_LABEL, ui);

        // 48 bus

        Text::new(format!("48 Bus").as_str())
            .x_y((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 220.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(H_48_LABEL, ui);

        let (h_48_v, h_48_v_color) =
            match self.h_48_v.get() {
                Some(v) => {
                    (format!("{0:.2}V", v), self.h_48_v_limits.get_color(v))
                },
                None => {
                    ("NO DATA".to_string(), rgb(0.0, 0.0, 0.0))
                },
            };
        Text::new(h_48_v.as_str())
            .x_y((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 240.0)
            .font_size(16)
            .color(h_48_v_color)
            .set(H_48_V_VALUE, ui);

        /*Text::new("NO DATA")
            .x_y((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 240.0)
            .font_size(16)
            .color(rgb(0.0, 0.0, 0.0))
            .set(H_48_A_VALUE, ui);*/

        // 24 bus

        Text::new(format!("24 H-Bus").as_str())
            .x_y((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 280.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(H_24_LABEL, ui);

        let (h_24_v, h_24_v_color) =
            match self.h_24_v.get() {
                Some(v) => {
                    (format!("{0:.2}V", v), self.h_24_v_limits.get_color(v))
                },
                None => {
                    ("NO DATA".to_string(), rgb(0.0, 0.0, 0.0))
                },
            };
        let (h_24_a, h_24_a_color) =
            match self.h_24_a.get() {
                Some(a) => {
                    (format!("{0:.2}A", a), rgb(0.0, 1.0, 0.0))
                },
                None => {
                    ("NO DATA".to_string(), rgb(0.0, 0.0, 0.0))
                },
            };
        Text::new(h_24_v.as_str())
            .x_y((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 300.0)
            .font_size(16)
            .color(h_24_v_color)
            .set(H_24_V_VALUE, ui);

        Text::new(h_24_a.as_str())
            .x_y((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 300.0)
            .font_size(16)
            .color(h_24_a_color)
            .set(H_24_A_VALUE, ui);

        // P-12 E bus

        Text::new(format!("P-12 E Bus").as_str())
            .x_y((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 340.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(P_12_E_LABEL, ui);

        let (p_12_e_v, p_12_e_v_color) =
            match self.p_12_e_v.get() {
                Some(v) => {
                    (format!("{0:.2}V", v), self.p_12_e_v_limits.get_color(v))
                },
                None => {
                    ("NO DATA".to_string(), rgb(0.0, 0.0, 0.0))
                },
            };
        let (p_12_e_a, p_12_e_a_color) =
            match self.p_12_e_a.get() {
                Some(a) => {
                    (format!("{0:.2}A", a), rgb(0.0, 1.0, 0.0))
                },
                None => {
                    ("NO DATA".to_string(), rgb(0.0, 0.0, 0.0))
                },
            };
        Text::new(p_12_e_v.as_str())
            .x_y((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 360.0)
            .font_size(16)
            .color(p_12_e_v_color)
            .set(P_12_E_V_VALUE, ui);

        Text::new(p_12_e_a.as_str())
            .x_y((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 360.0)
            .font_size(16)
            .color(p_12_e_a_color)
            .set(P_12_E_A_VALUE, ui);

        // P-12 PL bus

        Text::new(format!("P-12 PL Bus").as_str())
            .x_y((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 400.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(P_12_PL_LABEL, ui);

        let (p_12_pl_v, p_12_pl_v_color) =
            match self.p_12_pl_v.get() {
                Some(v) => {
                    (format!("{0:.2}V", v), self.p_12_pl_v_limits.get_color(v))
                },
                None => {
                    ("NO DATA".to_string(), rgb(0.0, 0.0, 0.0))
                },
            };
        Text::new(p_12_pl_v.as_str())
            .x_y((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 420.0)
            .font_size(16)
            .color(p_12_pl_v_color)
            .set(P_12_PL_V_VALUE, ui);

        /*Text::new(p_12_pl_a.as_str())
            .x_y((-ui.win_w / 2.0) + 160.0, (ui.win_h / 2.0) - 420.0)
            .font_size(16)
            .color(p_12_pl_v_color)
            .set(P_12_PL_A_VALUE, ui);*/

        // Left motor

        Text::new(format!("L Motor").as_str())
            .x_y((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 460.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(L_MOTOR_POWER_LABEL, ui);

        let (l_motor_amp, l_motor_amp_color) =
            match self.l_motor_amp.get() {
                Some(amp) => {
                    (format!("{0:.2}A", amp), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(0.0, 0.0, 0.0)),
            };
        Text::new(l_motor_amp.as_str())
            .x_y((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 480.0)
            .font_size(16)
            .color(l_motor_amp_color)
            .set(L_MOTOR_AMP_LABEL, ui);

        // Right motor

        Text::new(format!("R Motor").as_str())
            .x_y((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 520.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(R_MOTOR_POWER_LABEL, ui);

        let (r_motor_amp, r_motor_amp_color) =
            match self.r_motor_amp.get() {
                Some(amp) => {
                    (format!("{0:.2}A", amp), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(0.0, 0.0, 0.0)),
            };
        Text::new(r_motor_amp.as_str())
            .x_y((-ui.win_w / 2.0) + 60.0, (ui.win_h / 2.0) - 540.0)
            .font_size(16)
            .color(r_motor_amp_color)
            .set(R_MOTOR_AMP_LABEL, ui);

        ////////////////////////////////////////////////////////////////////////////////////////////
        // GPS section

        Text::new("GPS")
            .x_y((-ui.win_w / 2.0) + 410.0, (ui.win_h / 2.0) - 50.0)
            .font_size(22)
            .color(self.bg_color.plain_contrast())
            .set(GPS_LABEL, ui);
        
        // Latitude label
        let (latitude, latitude_color) =
            match self.latitude {
                Some(lat) => {
                    (format!("{0:.2} N", lat), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(0.0, 0.0, 0.0)),
            };
        Text::new(latitude.as_str())
            .x_y((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 75.0)
            .font_size(16)
            .color(latitude_color)
            .set(LATITUDE_LABEL, ui);

        // Longitude label
        let (longitude, longitude_color) =
            match self.longitude {
                Some(lng) => {
                    (format!("{0:.2} W", lng), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(0.0, 0.0, 0.0)),
            };
        Text::new(longitude.as_str())
            .x_y((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 95.0)
            .font_size(16)
            .color(longitude_color)
            .set(LONGITUDE_LABEL, ui);
        
        // Speed label
        let (speed, speed_color) =
            match self.speed {
                Some(speed) => {
                    (format!("{0:.2} m/s", speed), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(0.0, 0.0, 0.0)),
            };
        Text::new(speed.as_str())
            .x_y((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 115.0)
            .font_size(16)
            .color(speed_color)
            .set(SPEED_LABEL, ui);

        // Altitude label
        let (gps_altitude, gps_altitude_color) =
            match self.gps_altitude {
                Some(alt) => {
                    (format!("{0:.2} m", alt), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(0.0, 0.0, 0.0)),
            };
        Text::new(gps_altitude.as_str())
            .x_y((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 135.0)
            .font_size(16)
            .color(gps_altitude_color)
            .set(GPS_ALTITUDE_LABEL, ui);

        // Angle label
        let (angle, angle_color) =
            match self.angle {
                Some(angle) => {
                    (format!("{0:.2} deg", angle), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(0.0, 0.0, 0.0)),
            };
        Text::new(angle.as_str())
            .x_y((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 155.0)
            .font_size(16)
            .color(angle_color)
            .set(ANGLE_LABEL, ui);

        ////////////////////////////////////////////////////////////////////////////////////////////
        // Temp section

        Text::new("Temp")
            .x_y((-ui.win_w / 2.0) + 410.0, (ui.win_h / 2.0) - 190.0)
            .font_size(20)
            .color(self.bg_color.plain_contrast())
            .set(TEMP_LABEL, ui);

        // Left motor temp

        Text::new(format!("L Motor").as_str())
            .x_y((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 220.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(L_MOTOR_TEMP_LABEL, ui);

        let (l_motor_temp, l_motor_temp_color) =
            match self.l_motor_temp.get() {
                Some(temp) => {
                    (format!("{0:.2} C", temp), self.l_motor_temp_limits.get_color(temp))
                },
                None => ("NO DATA".to_string(), rgb(0.0, 0.0, 0.0)),
            };
        Text::new(l_motor_temp.as_str())
            .x_y((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 220.0)
            .font_size(16)
            .color(l_motor_temp_color)
            .set(L_MOTOR_C_LABEL, ui);

        // Right motor temp

        Text::new(format!("R Motor").as_str())
            .x_y((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 240.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(R_MOTOR_TEMP_LABEL, ui);

        let (r_motor_temp, r_motor_temp_color) =
            match self.r_motor_temp.get() {
                Some(temp) => {
                    (format!("{0:.2} C", temp), self.r_motor_temp_limits.get_color(temp))
                },
                None => ("NO DATA".to_string(), rgb(0.0, 0.0, 0.0)),
            };
        Text::new(r_motor_temp.as_str())
            .x_y((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 240.0)
            .font_size(16)
            .color(r_motor_temp_color)
            .set(R_MOTOR_C_LABEL, ui);

        // Upper avionics box temp

        Text::new(format!("Upper Avionics").as_str())
            .x_y((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 260.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(UPR_A_TEMP_LABEL, ui);

        let (upper_avionics_temp, upper_avionics_temp_color) =
            match self.upper_avionics_temp.get() {
                Some(temp) => {
                    (format!("{0:.2} C", temp), self.upper_avionics_temp_limits.get_color(temp))
                },
                None => ("NO DATA".to_string(), rgb(0.0, 0.0, 0.0)),
            };
        Text::new(upper_avionics_temp.as_str())
            .x_y((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 260.0)
            .font_size(16)
            .color(upper_avionics_temp_color)
            .set(UPR_A_TEMP_VALUE, ui);

        // Lower avionics box temp

        Text::new(format!("Lower Avionics").as_str())
            .x_y((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 280.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(LWR_A_TEMP_LABEL, ui);

        let (lower_avionics_temp, lower_avionics_temp_color) =
            match self.lower_avionics_temp.get() {
                Some(temp) => {
                    (format!("{0:.2} C", temp), self.lower_avionics_temp_limits.get_color(temp))
                },
                None => ("NO DATA".to_string(), rgb(0.0, 0.0, 0.0)),
            };
        Text::new(lower_avionics_temp.as_str())
            .x_y((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 280.0)
            .font_size(16)
            .color(lower_avionics_temp_color)
            .set(LWR_A_TEMP_VALUE, ui);

        // Ambient temp

        Text::new(format!("Ambient").as_str())
            .x_y((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 300.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(AMBIENT_TEMP_LABEL, ui);

        let (ambient_temp, ambient_temp_color) =
            match self.ambient_temp.get() {
                Some(temp) => {
                    (format!("{0:.2} C", temp), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(0.0, 0.0, 0.0)),
            };
        Text::new(ambient_temp.as_str())
            .x_y((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 300.0)
            .font_size(16)
            .color(ambient_temp_color)
            .set(AMBIENT_TEMP_VALUE, ui);

        ////////////////////////////////////////////////////////////////////////////////////////////
        // Weather section

        Text::new("Weather")
            .x_y((-ui.win_w / 2.0) + 410.0, (ui.win_h / 2.0) - 350.0)
            .font_size(20)
            .color(self.bg_color.plain_contrast())
            .set(WEATHER_LABEL, ui);

        // Wind speed

        Text::new(format!("Wind Speed").as_str())
            .x_y((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 380.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(WIND_LABEL, ui);

        let (wind_speed, wind_speed_color) =
            match self.wind_speed.get() {
                Some(wind_speed) => {
                    (format!("{0:.2} m/s", wind_speed), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(0.0, 0.0, 0.0)),
            };
        Text::new(wind_speed.as_str())
            .x_y((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 380.0)
            .font_size(16)
            .color(wind_speed_color)
            .set(WIND_VALUE, ui);

        // Altitude

        Text::new(format!("Altitude").as_str())
            .x_y((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 400.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(ALTITUDE_LABEL, ui);

        let (altitude, altitude_color) =
            match self.altitude {
                Some(alt) => {
                    (format!("{0:.2} ft", alt), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(0.0, 0.0, 0.0)),
            };
        Text::new(altitude.as_str())
            .x_y((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 400.0)
            .font_size(16)
            .color(altitude_color)
            .set(ALTITUDE_VALUE, ui);

        // Pressure

        Text::new(format!("Pressure").as_str())
            .x_y((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 420.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(PRESSURE_LABEL, ui);

        let (pressure, pressure_color) =
            match self.pressure {
                Some(pressure) => {
                    (format!("{0:.2} hPa", pressure), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(0.0, 0.0, 0.0)),
            };
        Text::new(pressure.as_str())
            .x_y((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 420.0)
            .font_size(16)
            .color(pressure_color)
            .set(PRESSURE_VALUE, ui);

        // Temp

        Text::new(format!("Temp").as_str())
            .x_y((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 440.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(WEATHER_TEMP_LABEL, ui);

        let (temp, temp_color) =
            match self.temp {
                Some(temp) => {
                    (format!("{0:.2} C", temp), rgb(0.0, 1.0, 0.0))
                },
                None => ("NO DATA".to_string(), rgb(0.0, 0.0, 0.0)),
            };
        Text::new(temp.as_str())
            .x_y((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 440.0)
            .font_size(16)
            .color(temp_color)
            .set(WEATHER_TEMP_VALUE, ui);

        ////////////////////////////////////////////////////////////////////////////////////////////
        // IMU section

        Text::new("IMU")
            .x_y((-ui.win_w / 2.0) + 410.0, (ui.win_h / 2.0) - 500.0)
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
                         "NO DATA".to_string(), rgb(0.0, 0.0, 0.0)),
            };

        // IMU pitch

        Text::new(format!("Pitch").as_str())
            .x_y((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 530.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(IMU_PITCH_LABEL, ui);

        Text::new(pitch.as_str())
            .x_y((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 530.0)
            .font_size(16)
            .color(imu_color)
            .set(IMU_PITCH_VALUE, ui);

        // IMU roll

        Text::new(format!("Roll").as_str())
            .x_y((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 550.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(IMU_ROLL_LABEL, ui);

        Text::new(roll.as_str())
            .x_y((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 550.0)
            .font_size(16)
            .color(imu_color)
            .set(IMU_ROLL_VALUE, ui);

        // IMU heading

        Text::new("Heading")
            .x_y((-ui.win_w / 2.0) + 360.0, (ui.win_h / 2.0) - 570.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(IMU_HEADING_LABEL, ui);

        Text::new(heading.as_str())
            .x_y((-ui.win_w / 2.0) + 500.0, (ui.win_h / 2.0) - 570.0)
            .font_size(16)
            .color(imu_color)
            .set(IMU_HEADING_VALUE, ui);

        // Trend graph labels
        Text::new("H-48 V")
            .x_y((ui.win_w / 2.0) - 405.0 - 80.0, (ui.win_h / 2.0) - 90.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(TREND_H_48V_LABEL, ui);
        Text::new("H-24 A")
            .x_y((ui.win_w / 2.0) - 405.0 - 80.0, (ui.win_h / 2.0) - 270.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(TREND_H_24A_LABEL, ui);
        Text::new("P-12 E V")
            .x_y((ui.win_w / 2.0) - 405.0 - 80.0, (ui.win_h / 2.0) - 450.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(TREND_P_12_E_V_LABEL, ui);
        Text::new("LR Motor Temp")
            .x_y((ui.win_w / 2.0) - 405.0 - 80.0, (ui.win_h / 2.0) - 630.0)
            .font_size(18)
            .color(self.bg_color.plain_contrast())
            .set(TREND_LR_MOTOR_TEMP_LABEL, ui);
    }

    pub fn handle_packet(&mut self, packet: String) {
        let packets = packet.split("|");
        
        for packet in packets {
            let packet_parts: Vec<String> = packet.split(":").map(|s| s.to_string()).collect();

            //println!("{:?}", packet_parts);

            match packet_parts[0].as_str() {
                "VOLT" => {
                    /////////////////////
                    self.h_48_v.add_value(packet_parts[1].parse().unwrap_or(0.0));
                    let h_48_v = self.h_48_v.get().unwrap_or(0.0);

                    let point_x = self.v48_graph.num_points(0) as f64;
                    self.v48_graph.add_point(0, point_x, h_48_v);

                    /////////////////////
                    self.h_24_v.add_value(packet_parts[2].parse().unwrap_or(0.0));

                    /////////////////////
                    self.p_12_e_v.add_value(packet_parts[3].parse().unwrap_or(0.0));
                    let p_12_e_v = self.p_12_e_v.get().unwrap_or(0.0);

                    let point_x = self.v12_graph.num_points(0) as f64;
                    self.v12_graph.add_point(0, point_x, p_12_e_v);

                    /////////////////////
                    self.p_12_pl_v.add_value(packet_parts[4].parse().unwrap_or(0.0));
                },
                "AMP" => {
                    self.l_motor_amp.add_value(packet_parts[1].parse().unwrap_or(0.0));
                    self.r_motor_amp.add_value(packet_parts[2].parse().unwrap_or(0.0));
                    self.p_12_e_a.add_value(packet_parts[3].parse().unwrap_or(0.0));
                    
                    // h-24
                    self.h_24_a.add_value(packet_parts[4].parse().unwrap_or(0.0));
                    let h_24_a = self.p_12_e_v.get().unwrap_or(0.0);

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
                    self.upper_avionics_temp.add_value(packet_parts[1].parse().unwrap_or(0.0));
                },
                "LWR_A_TEMP" => {
                    self.lower_avionics_temp.add_value(packet_parts[1].parse().unwrap_or(0.0));
                },
                "AMBIENT_TEMP" => {
                    self.ambient_temp.add_value(packet_parts[1].parse().unwrap_or(0.0));
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
                    let ax: f64 = packet_parts[1].parse().unwrap_or(0.0);
                    let ay: f64 = packet_parts[2].parse().unwrap_or(0.0);
                    let az: f64 = packet_parts[3].parse().unwrap_or(0.0);

                    let mx: f64 = packet_parts[7].parse().unwrap_or(0.0);
                    let my: f64 = packet_parts[8].parse().unwrap_or(0.0);
                    let mz: f64 = packet_parts[9].parse().unwrap_or(0.0);

                    let (ax, ay, az) = (ay, -az, ax);
                    let (mx, my, mz) = (my, -mz, mx);

                    let roll = f64::atan2(ay, az);
                    let pitch = f64::atan2(-ax, ay*f64::sin(roll) + az*f64::cos(roll));
                    let heading = f64::atan2(mz*f64::sin(roll) - my*f64::cos(roll),
                                             mx*f64::cos(pitch) + my*f64::sin(pitch)*f64::sin(roll) + mz*f64::sin(pitch)*f64::cos(roll));

                    let mut heading = heading.to_degrees();
                    let mut roll = roll.to_degrees() + 180.0;
                    let pitch = pitch.to_degrees();
                    if heading < 0.0 {
                        heading += 360.0;
                    }
                    if roll >= 180.0 {
                        roll -= 360.0;
                    }
                    heading = 360.0 - heading;
                    self.pitch_roll_heading = Some((pitch, roll, heading));
                },
                _ => { println!("WARNING: Unknown packet ID: {}", packet_parts[0]) },
            }
        }
    }

    pub fn on_key_pressed(&mut self, key: piston_window::Key) {
        match key {
            _ => { },
        }
    }

    pub fn on_key_released(&mut self, key: piston_window::Key) {
        match key {
            _ => { },
        }
    }
}

widget_ids! {
    // Widget IDs
    LOCAL_TIME,
    UTC_TIME,
    MISSION_TIME_LABEL,
    MISSION_START_BUTTON,
    MISSION_RESET_BUTTON,
    TIME_DELAY,

    // Power section
    POWER_LABEL,

    H_48_LABEL,
    H_48_V_VALUE,
    H_48_A_VALUE,

    H_24_LABEL,
    H_24_V_VALUE,
    H_24_A_VALUE,

    P_12_E_LABEL,
    P_12_E_V_VALUE,
    P_12_E_A_VALUE,

    P_12_PL_LABEL,
    P_12_PL_V_VALUE,
    P_12_PL_A_VALUE,

    L_MOTOR_POWER_LABEL,
    L_MOTOR_RPM_LABEL,
    L_MOTOR_AMP_LABEL,

    R_MOTOR_POWER_LABEL,
    R_MOTOR_RPM_LABEL,
    R_MOTOR_AMP_LABEL,

    // GPS section
    GPS_LABEL,
    LATITUDE_LABEL,
    LONGITUDE_LABEL,
    SPEED_LABEL,
    GPS_ALTITUDE_LABEL,
    ANGLE_LABEL,

    // Temp section
    TEMP_LABEL,

    L_MOTOR_TEMP_LABEL,
    L_MOTOR_C_LABEL,

    R_MOTOR_TEMP_LABEL,
    R_MOTOR_C_LABEL,

    UPR_A_TEMP_LABEL,
    UPR_A_TEMP_VALUE,

    LWR_A_TEMP_LABEL,
    LWR_A_TEMP_VALUE,

    AMBIENT_TEMP_LABEL,
    AMBIENT_TEMP_VALUE,

    // Weather section
    WEATHER_LABEL,

    WIND_LABEL,
    WIND_VALUE,

    ALTITUDE_LABEL,
    ALTITUDE_VALUE,

    PRESSURE_LABEL,
    PRESSURE_VALUE,

    WEATHER_TEMP_LABEL,
    WEATHER_TEMP_VALUE,

    // IMU section
    IMU_LABEL,

    IMU_PITCH_LABEL,
    IMU_PITCH_VALUE,

    IMU_ROLL_LABEL,
    IMU_ROLL_VALUE,

    IMU_HEADING_LABEL,
    IMU_HEADING_VALUE,

    // Trend graph labels
    TREND_H_48V_LABEL,
    TREND_H_24A_LABEL,
    TREND_P_12_E_V_LABEL,
    TREND_LR_MOTOR_TEMP_LABEL,
}
