use graphics::Context;
use opengl_graphics::GlGraphics;

pub struct Heading {
    angle: f64,
}

impl Heading {
    pub fn new() -> Heading {
        Heading {
            angle: 0.0,
        }
    }
    
    pub fn draw(&self, c: Context, gl: &mut GlGraphics) {
        use graphics::*;
        
        // Draw background rectangle
        Rectangle::new([0.3, 0.3, 1.0, 1.0])
            .draw([0.0, 0.0, 120.0, 120.0],
                  &c.draw_state, c.transform,
                  gl);

        // Draw background compass circle
        Ellipse::new([0.0, 0.0, 0.0, 1.0])
            .draw([0.0, 0.0, 120.0, 120.0],
                  &c.draw_state, c.transform,
                  gl);

        // Draw triangle pointer thing
        {
            let c = c.trans(60.0, 60.0); // Center the pointer in the circle
            let c = c.rot_deg(self.angle);
            Polygon::new([0.0, 1.0, 0.0, 1.0])
                .draw(&[[0.0, -42.0 + -16.0], [4.0, -42.0 + 4.0], [-4.0, -42.0 + 4.0]],
                      &c.draw_state, c.transform,
                      gl);
        }
    }

    pub fn set_angle(&mut self, angle: f64) {
        self.angle = angle;
    }
}
