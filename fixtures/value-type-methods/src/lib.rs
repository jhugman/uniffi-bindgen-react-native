/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

// Point: a record with constructors and methods.
// Includes a constructor named `new` to exercise collision avoidance
// (our TypeScript `new: create` alias must be suppressed).
#[derive(Debug, Clone, uniffi::Record)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[uniffi::export]
impl Point {
    // Constructor: replaces TypeScript `new: create` alias
    #[uniffi::constructor]
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    // Constructor: does NOT collide with our `create` helper
    #[uniffi::constructor]
    pub fn origin() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    // Methods
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
