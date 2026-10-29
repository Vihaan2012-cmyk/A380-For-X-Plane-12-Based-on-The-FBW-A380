use crate::geo::{bearing_to, distance_to, place_bearing_distance, Coordinates, Position};

pub fn global_to_airport_coordinates(airport_pos: Coordinates, coordinates: Coordinates) -> Position {
    let bearing = bearing_to(airport_pos, coordinates);
    let distance = distance_to(airport_pos, coordinates);
    let x_nm = distance * bearing.to_radians().cos();
    let y_nm = distance * bearing.to_radians().sin();
    let nm_to_meters = 1_000.0 / 0.539957;
    [y_nm * nm_to_meters, x_nm * nm_to_meters]
}

pub fn airport_to_global_coordinates(airport_pos: Coordinates, local: Position) -> Coordinates {
    let nm_to_meters = 1852.0;
    let y_nm = local[0] / nm_to_meters;
    let x_nm = local[1] / nm_to_meters;
    let distance_nm = x_nm.hypot(y_nm);
    let mut bearing_deg = y_nm.atan2(x_nm).to_degrees();
    if bearing_deg < 0.0 {
        bearing_deg += 360.0;
    }
    place_bearing_distance(airport_pos, bearing_deg, distance_nm)
}
