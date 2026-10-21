#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FitRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub fn fit_rect(
    source_width: f32,
    source_height: f32,
    target_width: f32,
    target_height: f32,
) -> FitRect {
    if source_width <= 0.0 || source_height <= 0.0 || target_width <= 0.0 || target_height <= 0.0 {
        return FitRect {
            x: 0.0,
            y: 0.0,
            width: target_width.max(1.0),
            height: target_height.max(1.0),
        };
    }
    let source_aspect = source_width / source_height;
    let target_aspect = target_width / target_height;
    if source_aspect > target_aspect {
        let height = target_width / source_aspect;
        FitRect {
            x: 0.0,
            y: (target_height - height) * 0.5,
            width: target_width,
            height,
        }
    } else {
        let width = target_height * source_aspect;
        FitRect {
            x: (target_width - width) * 0.5,
            y: 0.0,
            width,
            height: target_height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_content_gets_horizontal_bars() {
        let fitted = fit_rect(2000.0, 1000.0, 1000.0, 700.0);
        assert_eq!(fitted.x, 0.0);
        assert_eq!(fitted.y, 100.0);
        assert_eq!(fitted.width, 1000.0);
        assert_eq!(fitted.height, 500.0);
    }

    #[test]
    fn tall_content_gets_vertical_bars() {
        let fitted = fit_rect(1000.0, 2000.0, 1000.0, 700.0);
        assert_eq!(fitted.x, 325.0);
        assert_eq!(fitted.y, 0.0);
        assert_eq!(fitted.width, 350.0);
        assert_eq!(fitted.height, 700.0);
    }
}
