use graphics::Context;
use opengl_graphics::GlGraphics;
use opengl_graphics::glyph_cache::GlyphCache;

pub struct LineGraph {
    points: Vec<(f64, f64)>,
    pub size: (f64, f64),
    pub x_interval: (f64, f64),
    pub y_interval: (f64, f64),
}

impl LineGraph {
    pub fn new(size: (f64, f64), x_interval: (f64, f64), y_interval: (f64, f64)) -> LineGraph {
        LineGraph {
            points: Vec::new(),
            size: size,
            x_interval: x_interval,
            y_interval: y_interval,
        }
    }
    
    pub fn draw(&self, c: Context, gl: &mut GlGraphics, glyph_cache: &mut GlyphCache) {
        use graphics::*;
        
        Rectangle::new([0.3, 0.3, 1.0, 1.0])
            .draw([0.0, 0.0, self.size.0, self.size.1],
                  &c.draw_state, c.transform,
                  gl);
        
        for i in (1..self.points.len()) {
            let (x, y) = self.points[i];
            let (last_x, last_y) = self.points[i - 1];
            
            let x_norm = (x - self.x_interval.0)/(self.x_interval.1 - self.x_interval.0);
            let y_norm = (y - self.y_interval.0)/(self.y_interval.1 - self.y_interval.0);
            
            let last_x_norm = (last_x - self.x_interval.0)/(self.x_interval.1 - self.x_interval.0);
            let last_y_norm = (last_y - self.y_interval.0)/(self.y_interval.1 - self.y_interval.0);
            
            if x >= self.x_interval.0 && last_x <= self.x_interval.1 {
                Line::new([1.0, 0.0, 0.0, 1.0], 1.0)
                    .draw([last_x_norm * self.size.0, self.size.1 - last_y_norm*self.size.1,
                           x_norm * self.size.0, self.size.1 - y_norm*self.size.1],
                          &c.draw_state, c.transform,
                          gl);
            }
        }
    }
    
    pub fn add_point(&mut self, x: f64, y: f64) {
        if self.points.len() == 0 || self.points[self.points.len()-1].0 < x {
            self.points.push((x, y));
        }
    }
    
    pub fn num_points(&self) -> usize {
        self.points.len()
    }
}