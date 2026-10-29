pub trait Vars {
    fn get(&self, name: &str) -> f64;
}

#[derive(Debug, Clone, PartialEq)]
pub enum Out {
    Var(String, f64),
    Str(String, String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub t: f64,
    pub out: Out,
}
