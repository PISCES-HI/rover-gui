use graphics::{Context, Graphics};

// Roll

pub struct Roll {
    angle: f64,
}

impl Roll {
    pub fn new() -> Roll {
        Roll {
            angle: 0.0,
        }
    }
    
    pub fn draw<G: Graphics>(&self, c: Context, g: &mut G) {
        use graphics::*;
        
        // Draw background rectangle
        Rectangle::new([0.3, 0.3, 1.0, 1.0])
            .draw([0.0, 0.0, 120.0, 120.0],
                  &c.draw_state, c.transform,
                  g);

        // Draw base line
        Line::new([0.0, 0.0, 0.0, 1.0], 2.0)
            .draw([0.0, 60.0, 120.0, 60.0],
                  &c.draw_state, c.transform,
                  g);

        // Draw rotator line
        {
            let c = c.trans(60.0, 60.0); // Center the pointer in the circle
            let c = c.rot_deg(self.angle);
            Line::new([1.0, 0.0, 0.0, 1.0], 1.0)
                .draw([-60.0, 0.0, 60.0, 0.0],
                      &c.draw_state, c.transform,
                      g);
        }
    }

    pub fn set_angle(&mut self, angle: f64) {
        self.angle = angle;
    }
}

// Heading

pub struct Heading {
    angle: f64,
}

impl Heading {
    pub fn new() -> Heading {
        Heading {
            angle: 0.0,
        }
    }
    
    pub fn draw<G: Graphics>(&self, c: Context, g: &mut G) {
        use graphics::*;
        
        // Draw background rectangle
        Rectangle::new([0.3, 0.3, 1.0, 1.0])
            .draw([0.0, 0.0, 120.0, 120.0],
                  &c.draw_state, c.transform,
                  g);

        // Draw background compass circle
        Ellipse::new([0.0, 0.0, 0.0, 1.0])
            .draw([0.0, 0.0, 120.0, 120.0],
                  &c.draw_state, c.transform,
                  g);

        // Draw triangle pointer thing
        {
            let c = c.trans(60.0, 60.0); // Center the pointer in the circle
            let c = c.rot_deg(self.angle);
            Polygon::new([0.0, 1.0, 0.0, 1.0])
                .draw(&[[0.0, -42.0 + -16.0], [4.0, -42.0 + 4.0], [-4.0, -42.0 + 4.0]],
                      &c.draw_state, c.transform,
                      g);
        }
    }

    pub fn set_angle(&mut self, angle: f64) {
        self.angle = angle;
    }
}
