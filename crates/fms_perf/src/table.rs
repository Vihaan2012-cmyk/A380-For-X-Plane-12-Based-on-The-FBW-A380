#[derive(Clone, Debug)]
pub struct Table1 {
    xs: Vec<f64>,
    vs: Vec<f64>,
}

impl Table1 {
    pub fn new(rows: &[(f64, f64)]) -> Self {
        let mut r: Vec<(f64, f64)> = rows.to_vec();
        r.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        Table1 {
            xs: r.iter().map(|x| x.1).collect(),
            vs: r.iter().map(|x| x.0).collect(),
        }
    }

    pub fn get(&self, x: f64) -> f64 {
        interp(&self.xs, &self.vs, x)
    }
}

#[derive(Clone, Debug)]
pub struct Table2 {
    xs: Vec<f64>,
    ys: Vec<f64>,
    v: Vec<Vec<f64>>,
}

impl Table2 {
    pub fn new(rows: &[(f64, f64, f64)]) -> Self {
        let mut xs: Vec<f64> = rows.iter().map(|r| r.1).collect();
        let mut ys: Vec<f64> = rows.iter().map(|r| r.2).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        xs.dedup();
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        ys.dedup();
        let mut v = vec![vec![f64::NAN; ys.len()]; xs.len()];
        for r in rows {
            let xi = xs.iter().position(|x| *x == r.1).unwrap();
            let yi = ys.iter().position(|y| *y == r.2).unwrap();
            v[xi][yi] = r.0;
        }
        Table2 { xs, ys, v }
    }

    pub fn get(&self, x: f64, y: f64) -> f64 {
        let col: Vec<f64> = self.v.iter().map(|row| interp(&self.ys, row, y)).collect();
        interp(&self.xs, &col, x)
    }
}

pub fn interp(xs: &[f64], vs: &[f64], x: f64) -> f64 {
    if x <= xs[0] {
        return vs[0];
    }
    let last = xs.len() - 1;
    if x >= xs[last] {
        return vs[last];
    }
    let mut i = 0;
    while x > xs[i + 1] {
        i += 1;
    }
    let t = (x - xs[i]) / (xs[i + 1] - xs[i]);
    vs[i] + (vs[i + 1] - vs[i]) * t
}

pub fn interp_pairs(table: &[(f64, f64)], x: f64) -> f64 {
    let xs: Vec<f64> = table.iter().map(|p| p.0).collect();
    let vs: Vec<f64> = table.iter().map(|p| p.1).collect();
    interp(&xs, &vs, x)
}
