/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

// Point: a record with methods.
// NOTE: Record constructors via #[uniffi::constructor] are not yet supported in uniffi 0.31.0
// because the metadata reader (uniffi_meta::reader::read_constructor) only accepts
// Type::Object return types, not Type::Record. This is a bug in uniffi 0.31.0.
// The TypeScript template for constructors is in place; this fixture tests methods only.
#[derive(Debug, Clone, uniffi::Record)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[uniffi::export]
impl Point {
    pub fn distance_to(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    pub fn scale(&self, factor: f64) -> Point {
        Point {
            x: self.x * factor,
            y: self.y * factor,
        }
    }
}

// Direction: a flat enum with methods
#[derive(Debug, Clone, uniffi::Enum)]
pub enum Direction {
    North,
    South,
    East,
    West,
}

#[uniffi::export]
impl Direction {
    pub fn opposite(&self) -> Direction {
        match self {
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::East => Direction::West,
            Direction::West => Direction::East,
        }
    }

    pub fn is_vertical(&self) -> bool {
        matches!(self, Direction::North | Direction::South)
    }
}

// Shape: a tagged enum with methods
#[derive(Debug, Clone, uniffi::Enum)]
pub enum Shape {
    Circle { radius: f64 },
    Rectangle { width: f64, height: f64 },
}

#[uniffi::export]
impl Shape {
    pub fn area(&self) -> f64 {
        match self {
            Shape::Circle { radius } => std::f64::consts::PI * radius * radius,
            Shape::Rectangle { width, height } => width * height,
        }
    }
}

uniffi::setup_scaffolding!();
