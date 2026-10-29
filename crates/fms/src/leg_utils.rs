pub fn merge_leg_sets<T: Clone>(existing: &[T], incoming: &[T], downstream: bool, icao_code: impl Fn(&T) -> &str) -> Option<Vec<T>> {
    let mut final_legs: Option<Vec<T>> = None;

    if downstream {
        let mut connection_found = false;
        for existing_leg in existing {
            if connection_found {
                if let Some(legs) = final_legs.as_mut() {
                    legs.push(existing_leg.clone());
                    continue;
                }
            }

            let matching = incoming.iter().position(|leg| icao_code(leg) == icao_code(existing_leg));
            if let Some(index) = matching {
                let mut legs = incoming.to_vec();
                connection_found = true;
                legs.truncate(index);
                legs.push(existing_leg.clone());
                final_legs = Some(legs);
            }
        }
    } else {
        for i in (0..existing.len()).rev() {
            let existing_leg = &existing[i];
            let matching = incoming.iter().position(|leg| icao_code(leg) == icao_code(existing_leg));
            if let Some(index) = matching {
                let mut legs = existing.to_vec();
                legs.truncate(i + 1);
                legs.extend(incoming[index + 1..].iter().cloned());
                final_legs = Some(legs);
                break;
            }
        }
    }

    final_legs
}
