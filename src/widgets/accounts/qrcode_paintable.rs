use gtk::{gdk, glib, graphene, prelude::*, subclass::prelude::*};

#[allow(clippy::upper_case_acronyms)]
pub struct QRCodeData {
    pub width: i32,
    pub height: i32,
    pub items: Vec<Vec<bool>>,
}

impl<B: AsRef<[u8]>> From<B> for QRCodeData {
    fn from(data: B) -> Self {
        let code = qrencode::QrCode::new(data).unwrap();
        let items = code
            .render::<char>()
            .quiet_zone(false)
            .module_dimensions(1, 1)
            .build()
            .split('\n')
            .map(|line| {
                line.chars()
                    .map(|c| !c.is_whitespace())
                    .collect::<Vec<bool>>()
            })
            .collect::<Vec<Vec<bool>>>();

        let width = items.first().unwrap().len() as i32;
        let height = items.len() as i32;
        Self {
            width,
            height,
            items,
        }
    }
}

mod imp {
    use std::cell::RefCell;

    use super::*;

    #[allow(clippy::upper_case_acronyms)]
    #[derive(Default)]
    pub struct QRCodePaintable {
        pub qrcode: RefCell<Option<QRCodeData>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for QRCodePaintable {
        const NAME: &'static str = "QRCodePaintable";
        type Type = super::QRCodePaintable;
        type Interfaces = (gdk::Paintable,);
    }

    impl ObjectImpl for QRCodePaintable {}
    impl PaintableImpl for QRCodePaintable {
        fn snapshot(&self, snapshot: &gdk::Snapshot, width: f64, height: f64) {
            if let Some(ref qrcode) = *self.qrcode.borrow() {
                let padding_squares = 3.max(qrcode.height / 10);
                let square_height = height as f32 / (qrcode.height + 2 * padding_squares) as f32;
                let square_width = width as f32 / (qrcode.width + 2 * padding_squares) as f32;
                let padding = square_height * padding_squares as f32;

                let rect = graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
                snapshot.append_color(&gdk::RGBA::WHITE, &rect);
                qrcode.items.iter().enumerate().for_each(|(y, line)| {
                    line.iter().enumerate().for_each(|(x, is_dark)| {
                        if *is_dark {
                            let mut black = gdk::RGBA::BLACK;
                            black.set_alpha(0.85);

                            let position = graphene::Rect::new(
                                (x as f32) * square_width + padding,
                                (y as f32) * square_height + padding,
                                square_width,
                                square_height,
                            );

                            snapshot.append_color(&black, &position);
                        };
                    });
                });
            }
        }
    }
}

glib::wrapper! {
    pub struct QRCodePaintable(ObjectSubclass<imp::QRCodePaintable>)
        @implements gdk::Paintable;
}

impl QRCodePaintable {
    pub fn set_qrcode(&self, qrcode: QRCodeData) {
        self.imp().qrcode.replace(Some(qrcode));
        self.invalidate_contents();
    }
}

impl Default for QRCodePaintable {
    fn default() -> Self {
        glib::Object::new()
    }
}
