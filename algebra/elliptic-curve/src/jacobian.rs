use crate::{curve::Affine, curve_operations};
use std::{
    ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign},
    prelude::v1::*,
};
use zkp_primefield::FieldElement;
use zkp_u256::{commutative_binop, noncommutative_binop, U256};

// See http://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian.html

#[derive(Clone)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Jacobian {
    pub x: FieldElement,
    pub y: FieldElement,
    pub z: FieldElement,
}

impl Jacobian {
    pub const ZERO: Self = Self {
        x: FieldElement::ONE,
        y: FieldElement::ONE,
        z: FieldElement::ZERO,
    };

    pub fn on_curve(&self) -> bool {
        // TODO: Compute without inverting Z
        Affine::from(self).on_curve()
    }

    pub fn double_assign(&mut self) {
        if self.y == FieldElement::ZERO {
            *self = Self::ZERO;
            return;
        }
        // OPT: Special case z == FieldElement::ONE?
        // See http://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian.html#doubling-dbl-2007-bl
        let xx = self.x.square();
        let yy = self.y.square();
        let yyyy = yy.square();
        let zz = self.z.square();
        let s = ((&self.x + &yy).square() - &xx - &yyyy).double();
        let m = xx.triple() + /* ALPHA * */ zz.square();
        self.z = (&self.y + &self.z).square() - yy - zz;
        self.x = m.square() - s.double();
        self.y = m * (s - &self.x) - yyyy.double().double().double(); // TODO: .octuple()
    }

    pub fn neg_assign(&mut self) {
        self.y.neg_assign();
    }

    pub fn double(&self) -> Self {
        let mut r = self.clone();
        r.double_assign();
        r
    }

    // Multiply Affine point using Jacobian accumulator
    pub fn mul(p: &Affine, scalar: &U256) -> Self {
        let mut r = Self::from(p);
        for i in (0..scalar.msb()).rev() {
            r.double_assign();
            if scalar.bit(i) {
                r += p;
            }
        }
        r
    }
}

impl PartialEq for Jacobian {
    fn eq(&self, rhs: &Self) -> bool {
        // TODO: without inverting Z
        Affine::from(self) == Affine::from(rhs)
    }
}

impl Default for Jacobian {
    fn default() -> Self {
        Self::ZERO
    }
}

impl From<&Affine> for Jacobian {
    fn from(other: &Affine) -> Self {
        match other {
            Affine::Zero => Self::ZERO,
            Affine::Point { x, y } => {
                Self {
                    x: x.clone(),
                    y: y.clone(),
                    z: FieldElement::ONE,
                }
            }
        }
    }
}

impl From<Affine> for Jacobian {
    fn from(other: Affine) -> Self {
        match other {
            Affine::Zero => Self::ZERO,
            Affine::Point { x, y } => {
                Self {
                    x,
                    y,
                    z: FieldElement::ONE,
                }
            }
        }
    }
}

impl From<&Jacobian> for Affine {
    fn from(other: &Jacobian) -> Self {
        match other.z.inv() {
            None => Self::ZERO,
            Some(zi) => {
                let zi2 = zi.square();
                let zi3 = zi * &zi2;
                Self::Point {
                    x: &other.x * zi2,
                    y: &other.y * zi3,
                }
            }
        }
    }
}

impl Neg for &Jacobian {
    type Output = Jacobian;

    fn neg(self) -> Jacobian {
        let mut r = self.clone();
        r.neg_assign();
        r
    }
}

impl AddAssign<&Jacobian> for Jacobian {
    // We want to use the variable naming convention from the source
    #[allow(clippy::many_single_char_names)]
    // We need multiplications to implement addition
    #[allow(clippy::suspicious_op_assign_impl)]
    fn add_assign(&mut self, rhs: &Self) {
        if rhs.z == FieldElement::ZERO {
            return;
        }
        if self.z == FieldElement::ZERO {
            // OPT: In non-assign move add, take rhs.
            *self = rhs.clone();
            return;
        }
        // OPT: Special case z == FieldElement::ONE?
        // See http://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian.html#addition-add-2007-bl
        let z1z1 = self.z.square();
        let z2z2 = rhs.z.square();
        let u1 = &self.x * &z2z2;
        let u2 = &rhs.x * &z1z1;
        let s1 = &self.y * &rhs.z * &z2z2;
        let s2 = &rhs.y * &self.z * &z1z1;
        if u1 == u2 {
            return if s1 == s2 {
                self.double_assign()
            } else {
                *self = Self::ZERO
            };
        }
        let h = u2 - &u1;
        let i = h.double().square();
        let j = &h * &i;
        let r = (s2 - &s1).double();
        let v = u1 * i;
        self.x = r.square() - &j - v.double();
        self.y = r * (v - &self.x) - (s1 * j).double();
        self.z = ((&self.z + &rhs.z).square() - z1z1 - z2z2) * h;
    }
}

impl AddAssign<&Affine> for Jacobian {
    // We want to use the variable naming convention from the source
    #[allow(clippy::many_single_char_names)]
    // We need multiplications to implement addition
    #[allow(clippy::suspicious_op_assign_impl)]
    fn add_assign(&mut self, rhs: &Affine) {
        match rhs {
            Affine::Zero => { /* Do nothing */ }
            Affine::Point { x, y } => {
                if self.z == FieldElement::ZERO {
                    self.x = x.clone();
                    self.y = y.clone();
                    self.z = FieldElement::ONE;
                    return;
                }
                // OPT: Special case z == FieldElement::ONE?
                // See http://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian.html#addition-madd-2007-bl
                let z1z1 = self.z.square();
                let u2 = x * &z1z1;
                let s2 = y * &self.z * &z1z1;
                if self.x == u2 {
                    return if self.x == s2 {
                        self.double_assign()
                    } else {
                        *self = Self::ZERO
                    };
                }
                let h = u2 - &self.x;
                let hh = h.square();
                let i = hh.double().double(); // TODO .quadruple()
                let j = &h * &i;
                let r = (s2 - &self.y).double();
                let v = &self.x * i;
                self.x = r.square() - &j - v.double();
                self.y = r * (v - &self.x) - (&self.y * j).double();
                self.z = (&self.z + h).square() - z1z1 - hh;
            }
        }
    }
}

// TODO: Various Add implementations mixing Affine and Jacobian values and refs.
impl Add<&Affine> for &Jacobian {
    type Output = Jacobian;

    fn add(self, rhs: &Affine) -> Jacobian {
        let mut r = self.clone();
        r += rhs;
        r
    }
}

impl SubAssign<&Affine> for Jacobian {
    fn sub_assign(&mut self, rhs: &Affine) {
        self.add_assign(&rhs.neg())
    }
}

curve_operations!(Jacobian);
commutative_binop!(Jacobian, Add, add, AddAssign, add_assign);
noncommutative_binop!(Jacobian, Sub, sub, SubAssign, sub_assign);

#[cfg(test)]
use quickcheck::{Arbitrary, Gen};

#[cfg(test)]
impl Arbitrary for Jacobian {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        // To force Z to be non trivial we add two points.
        let mut r = Self::from(Affine::arbitrary(g));
        r += &Affine::arbitrary(g);
        r
    }
}

// TODO: Replace literals with u256h!
#[allow(clippy::unreadable_literal)]
// Quickcheck needs pass by value
#[allow(clippy::needless_pass_by_value)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ORDER;
    use quickcheck_macros::quickcheck;
    use zkp_macros_decl::u256h;

    #[test]
    fn test_add() {
        let a = Jacobian::from(Affine::new(
            FieldElement::from(u256h!(
                "01ef15c18599971b7beced415a40f0c7deacfd9b0d1819e03d723d8bc943cfca"
            )),
            FieldElement::from(u256h!(
                "005668060aa49730b7be4801df46ec62de53ecd11abe43a32873000c36e8dc1f"
            )),
        ));
        let b = Jacobian::from(Affine::new(
            FieldElement::from(u256h!(
                "00f24921907180cd42c9d2d4f9490a7bc19ac987242e80ac09a8ac2bcf0445de"
            )),
            FieldElement::from(u256h!(
                "018a7a2ab4e795405f924de277b0e723d90eac55f2a470d8532113d735bdedd4"
            )),
        ));
        let c = Jacobian::from(Affine::new(
            FieldElement::from(u256h!(
                "0457342950d2475d9e83a4de8beb3c0850181342ea04690d804b37aa907b735f"
            )),
            FieldElement::from(u256h!(
                "00011bd6102b929632ce605b5ae1c9c6c1b8cba2f83aa0c5a6d1247318871137"
            )),
        ));
        assert_eq!(a + b, c);
    }

    #[test]
    fn test_double() {
        let a = Jacobian::from(Affine::new(
            FieldElement::from(u256h!(
                "01ef15c18599971b7beced415a40f0c7deacfd9b0d1819e03d723d8bc943cfca"
            )),
            FieldElement::from(u256h!(
                "005668060aa49730b7be4801df46ec62de53ecd11abe43a32873000c36e8dc1f"
            )),
        ));
        let b = Jacobian::from(Affine::new(
            FieldElement::from(u256h!(
                "0759ca09377679ecd535a81e83039658bf40959283187c654c5416f439403cf5"
            )),
            FieldElement::from(u256h!(
                "06f524a3400e7708d5c01a28598ad272e7455aa88778b19f93b562d7a9646c41"
            )),
        ));
        assert_eq!(a.double(), b);
    }

    #[test]
    fn test_mul() {
        let a = Jacobian::from(Affine::new(
            FieldElement::from(u256h!(
                "01ef15c18599971b7beced415a40f0c7deacfd9b0d1819e03d723d8bc943cfca"
            )),
            FieldElement::from(u256h!(
                "005668060aa49730b7be4801df46ec62de53ecd11abe43a32873000c36e8dc1f"
            )),
        ));
        let b = u256h!("07374b7d69dc9825fc758b28913c8d2a27be5e7c32412f612b20c9c97afbe4dd");
        let c = Jacobian::from(Affine::new(
            FieldElement::from(u256h!(
                "00f24921907180cd42c9d2d4f9490a7bc19ac987242e80ac09a8ac2bcf0445de"
            )),
            FieldElement::from(u256h!(
                "018a7a2ab4e795405f924de277b0e723d90eac55f2a470d8532113d735bdedd4"
            )),
        ));
        assert_eq!(a * b, c);
    }

    #[allow(clippy::eq_op)]
    #[quickcheck]
    fn add_commutative(a: Jacobian, b: Jacobian) -> bool {
        &a + &b == &b + &a
    }

    #[quickcheck]
    fn distributivity(p: Jacobian, mut a: U256, mut b: U256) -> bool {
        a %= &ORDER;
        b %= &ORDER;
        let c = &a + &b;
        // TODO: c %= &ORDER;
        (&p * a) + (&p * b) == &p * c
    }
}
