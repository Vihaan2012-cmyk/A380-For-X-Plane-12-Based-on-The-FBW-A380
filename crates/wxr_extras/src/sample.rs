#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point3 {
    pub east_m: f64,
    pub north_m: f64,
    pub alt_m: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WeatherPoint {
    pub wind_east_ms: f32,
    pub wind_north_ms: f32,
    pub vertical_ms: f32,
    pub turbulence: f32,
    pub precip: f32,
    pub temp_c: Option<f32>,
    pub cloud_top_m: Option<f32>,
}

pub trait WeatherSampler {
    fn sample(&mut self, p: Point3) -> Option<WeatherPoint>;
}

impl<F: FnMut(Point3) -> Option<WeatherPoint>> WeatherSampler for F {
    fn sample(&mut self, p: Point3) -> Option<WeatherPoint> {
        self(p)
    }
}
