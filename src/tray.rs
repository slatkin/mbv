use ksni::blocking::TrayMethods;

const TRAY_ICON: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tray_icon.bin"));

struct MbvTray {
    shutdown_tx: std::sync::mpsc::SyncSender<()>,
}

impl ksni::Tray for MbvTray {
    fn id(&self) -> String {
        "mbv".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![ksni::Icon {
            width: 24,
            height: 24,
            data: TRAY_ICON.to_vec(),
        }]
    }

    fn title(&self) -> String {
        "mbv".into()
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![StandardItem {
            label: "Quit".into(),
            icon_name: "application-exit".into(),
            activate: Box::new(|tray: &mut Self| {
                let _ = tray.shutdown_tx.try_send(());
            }),
            ..Default::default()
        }
        .into()]
    }
}

pub fn spawn(shutdown_tx: std::sync::mpsc::SyncSender<()>) -> Option<Box<dyn Send>> {
    MbvTray { shutdown_tx }
        .spawn()
        .map(|tray| Box::new(tray) as Box<dyn Send>)
        .map_err(|e| {
            log::warn!(target: "tray", "not available: {e}");
        })
        .ok()
}
