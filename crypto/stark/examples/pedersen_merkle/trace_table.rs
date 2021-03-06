use super::{
    inputs::{Claim, Witness},
    pedersen_points::{PEDERSEN_POINTS, SHIFT_POINT},
};
use std::prelude::v1::*;
use zkp_elliptic_curve::Affine;
use zkp_primefield::FieldElement;
use zkp_stark::TraceTable;
use zkp_u256::U256;

// TODO: Naming
#[allow(clippy::module_name_repetitions)]
pub fn get_trace_table(claim: &Claim, witness: &Witness) -> TraceTable {
    let num_columns = 8;
    let mut trace = TraceTable::new(claim.path_length * 256, num_columns);

    let mut row: Row = Row::default();
    row.right.point = Affine::Point {
        x: claim.leaf.clone(),
        y: FieldElement::ZERO,
    };

    for path_index in 0..claim.path_length {
        for bit_index in 0..256 {
            if bit_index % 256 == 0 {
                let other_hash = U256::from(&witness.path[path_index]);
                let (x, _) = get_coordinates(&row.right.point);
                if witness.directions[path_index] {
                    row = initialize_hash(other_hash, U256::from(x));
                } else {
                    row = initialize_hash(U256::from(x), other_hash);
                }
            } else {
                row = hash_next_bit(&row, bit_index);
            }
            let row_index = path_index * 256 + bit_index;

            let (left_x, left_y) = get_coordinates(&row.left.point);
            trace[(row_index, 0)] = FieldElement::from(row.left.source.clone());
            trace[(row_index, 1)] = row.left.slope.clone();
            trace[(row_index, 2)] = left_x.clone();
            trace[(row_index, 3)] = left_y.clone();

            let (right_x, right_y) = get_coordinates(&row.right.point);
            trace[(row_index, 4)] = FieldElement::from(row.right.source.clone());
            trace[(row_index, 5)] = row.right.slope.clone();
            trace[(row_index, 6)] = right_x.clone();
            trace[(row_index, 7)] = right_y.clone();
        }
    }
    trace
}

fn initialize_hash(left_source: U256, right_source: U256) -> Row {
    let mut row: Row = Row::default();
    row.left.source = left_source;
    row.right.source = right_source;
    row.right.point = SHIFT_POINT;
    row
}

fn hash_next_bit(row: &Row, bit_index: usize) -> Row {
    let mut next_row = Row {
        left:  Subrow {
            source: row.left.source.clone() >> 1,
            point: row.right.point.clone(),
            ..Subrow::default()
        },
        right: Subrow {
            source: row.right.source.clone() >> 1,
            ..Subrow::default()
        },
    };
    if row.left.source.bit(0) {
        let p = &PEDERSEN_POINTS[bit_index];
        next_row.left.slope = get_slope(&next_row.left.point, &p);
        next_row.left.point += p;
    }

    next_row.right.point = next_row.left.point.clone();
    if row.right.source.bit(0) {
        let p = &PEDERSEN_POINTS[bit_index + 252];
        next_row.right.slope = get_slope(&next_row.right.point, &p);
        next_row.right.point += p;
    }
    next_row
}

#[derive(Default)]
struct Row {
    left:  Subrow,
    right: Subrow,
}

struct Subrow {
    source: U256,
    slope:  FieldElement,
    point:  Affine,
}

impl Default for Subrow {
    fn default() -> Self {
        Self {
            source: U256::ZERO,
            slope:  FieldElement::ZERO,
            point:  Affine::Point {
                x: FieldElement::ZERO,
                y: FieldElement::ZERO,
            },
        }
    }
}

fn get_slope(p_1: &Affine, p_2: &Affine) -> FieldElement {
    let (x_1, y_1) = get_coordinates(p_1);
    let (x_2, y_2) = get_coordinates(p_2);
    (y_1 - y_2) / (x_1 - x_2)
}

fn get_coordinates(p: &Affine) -> (&FieldElement, &FieldElement) {
    match p {
        Affine::Zero => panic!(),
        Affine::Point { x, y } => (x, y),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        super::inputs::{short_witness, SHORT_CLAIM},
        *,
    };

    #[test]
    fn short_inputs_consistent() {
        let trace = get_trace_table(&SHORT_CLAIM, &short_witness());
        assert_eq!(trace[(trace.num_rows() - 1, 6)], SHORT_CLAIM.root);
    }
}
