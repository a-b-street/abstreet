use crate::GeomBatch;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// Similar to [`Widget::row`]/[`Widget::column`], but for [`GeomBatch`]s instead of [`Widget`]s,
/// and follows a builder pattern
///
/// You can add items incrementally, change `spacing` and `axis`, and call `batch` at the end to
/// apply these rules to produce an aggeregate `GeomBatch`.
#[derive(Debug)]
pub struct GeomBatchStack {
    batches: Vec<GeomBatch>,
    axis: Axis,
    spacing: f64,
}

impl Default for GeomBatchStack {
    fn default() -> Self {
        GeomBatchStack {
            batches: vec![],
            // TODO:
            // alignment: Alignment::Center,
            axis: Axis::Horizontal,
            spacing: 0.0,
        }
    }
}

impl GeomBatchStack {
    pub fn horizontal(batches: Vec<GeomBatch>) -> Self {
        GeomBatchStack {
            batches,
            axis: Axis::Horizontal,
            ..Default::default()
        }
    }

    pub fn vertical(batches: Vec<GeomBatch>) -> Self {
        GeomBatchStack {
            batches,
            axis: Axis::Vertical,
            ..Default::default()
        }
    }

    pub fn set_axis(&mut self, new_value: Axis) {
        self.axis = new_value;
    }

    pub fn push(&mut self, geom_batch: GeomBatch) {
        self.batches.push(geom_batch);
    }

    pub fn append(&mut self, geom_batches: &mut Vec<GeomBatch>) {
        self.batches.append(geom_batches);
    }

    pub fn spacing(&mut self, spacing: f64) -> &mut Self {
        self.spacing = spacing;
        self
    }

    pub fn batch(self) -> GeomBatch {
        if self.batches.is_empty() {
            return GeomBatch::new();
        }

        let max_bound_for_axis = self
            .batches
            .iter()
            .map(GeomBatch::get_bounds)
            .max_by(|b1, b2| match self.axis {
                Axis::Vertical => b1.width().partial_cmp(&b2.width()).unwrap(),
                Axis::Horizontal => b1.height().partial_cmp(&b2.height()).unwrap(),
            })
            .unwrap();

        let mut stack_batch = GeomBatch::new();
        let mut stack_offset = 0.0;
        for mut batch in self.batches {
            let bounds = batch.get_bounds();
            let alignment_inset = match self.axis {
                Axis::Vertical => (max_bound_for_axis.width() - bounds.width()) / 2.0,
                Axis::Horizontal => (max_bound_for_axis.height() - bounds.height()) / 2.0,
            };

            let (dx, dy) = match self.axis {
                Axis::Vertical => (alignment_inset, stack_offset),
                Axis::Horizontal => (stack_offset, alignment_inset),
            };
            batch = batch.translate(dx, dy);
            stack_batch.append(batch);

            stack_offset += self.spacing;
            match self.axis {
                Axis::Vertical => stack_offset += bounds.height(),
                Axis::Horizontal => stack_offset += bounds.width(),
            }
        }
        stack_batch
    }
}
