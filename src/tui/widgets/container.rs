//! A flex container: splits an area along one axis into parts that are a fixed size or share
//! what is left. It draws nothing and has no margin, padding or gap, so parts sit edge to edge
//! and together cover the area. Whatever a part holds is the caller's to draw.

use ratatui::layout::{Constraint, Direction, Layout, Rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    /// This many cells along the axis; empty when it is 0.
    Fixed(u16),
    /// A share of what the fixed parts leave; `Fill(2)` takes twice what `Fill(1)` does.
    Fill(u16),
}

impl From<Size> for Constraint {
    fn from(size: Size) -> Self {
        match size {
            Size::Fixed(cells) => Constraint::Length(cells),
            Size::Fill(share) => Constraint::Fill(share),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Container<const N: usize> {
    axis: Direction,
    sizes: [Size; N],
}

impl<const N: usize> Container<N> {
    /// Parts stacked top to bottom.
    pub fn column(sizes: [Size; N]) -> Self {
        Container {
            axis: Direction::Vertical,
            sizes,
        }
    }

    /// Parts side by side, left to right.
    pub fn row(sizes: [Size; N]) -> Self {
        Container {
            axis: Direction::Horizontal,
            sizes,
        }
    }

    /// The area of each part, in order.
    pub fn areas(&self, area: Rect) -> [Rect; N] {
        Layout::new(self.axis, self.sizes.map(Constraint::from)).areas(area)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_column_gives_fixed_parts_their_size_and_fill_the_rest_with_no_gaps() {
        let [top, middle, bottom] =
            Container::column([Size::Fixed(1), Size::Fill(1), Size::Fixed(2)])
                .areas(Rect::new(3, 4, 20, 10));

        assert_eq!(top, Rect::new(3, 4, 20, 1));
        assert_eq!(middle, Rect::new(3, 5, 20, 7));
        assert_eq!(bottom, Rect::new(3, 12, 20, 2));
    }

    #[test]
    fn a_row_splits_the_width_and_fill_parts_share_in_proportion() {
        let [left, right] =
            Container::row([Size::Fill(1), Size::Fill(2)]).areas(Rect::new(0, 0, 30, 5));

        assert_eq!((left.width, right.width), (10, 20));
        assert_eq!(right.x, left.right());
    }

    #[test]
    fn a_part_of_no_cells_is_empty_and_the_others_still_cover_the_area() {
        let [none, rest] =
            Container::column([Size::Fixed(0), Size::Fill(1)]).areas(Rect::new(0, 0, 8, 6));

        assert!(none.is_empty());
        assert_eq!(rest, Rect::new(0, 0, 8, 6));
    }
}
