//! Geometry types used by the host API. All values are CSS pixels (`f32`), with the y axis
//! pointing downwards.

use std::ops::{Add, Sub};

/// A point (or offset) in CSS pixels. The y coordinate increases downwards.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Point = Point { x: 0.0, y: 0.0 };

    pub fn new(x: f32, y: f32) -> Self {
        Point { x, y }
    }
}

impl Add for Point {
    type Output = Point;
    fn add(self, rhs: Point) -> Point {
        Point::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Point {
    type Output = Point;
    fn sub(self, rhs: Point) -> Point {
        Point::new(self.x - rhs.x, self.y - rhs.y)
    }
}

/// A size in CSS pixels.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub fn new(width: f32, height: f32) -> Self {
        Size { width, height }
    }
}

/// An axis-aligned rectangle in CSS pixels.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Rect {
    /// The top left corner.
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Rect {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }

    pub fn right(&self) -> f32 {
        self.origin.x + self.size.width
    }

    pub fn bottom(&self) -> f32 {
        self.origin.y + self.size.height
    }
}

/// Values for the four sides of a box, e.g. padding, border widths or margins.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Edges<T> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

impl<T: Copy> Edges<T> {
    pub fn uniform(value: T) -> Self {
        Edges {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }
}

impl Edges<f32> {
    /// `left + right`
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    /// `top + bottom`
    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }

    pub fn is_zero(&self) -> bool {
        self.top == 0.0 && self.right == 0.0 && self.bottom == 0.0 && self.left == 0.0
    }
}

impl Add for Edges<f32> {
    type Output = Edges<f32>;
    fn add(self, rhs: Edges<f32>) -> Edges<f32> {
        Edges {
            top: self.top + rhs.top,
            right: self.right + rhs.right,
            bottom: self.bottom + rhs.bottom,
            left: self.left + rhs.left,
        }
    }
}
