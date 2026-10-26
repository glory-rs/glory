//! Permission requests happen only from these explicit buttons. Startup queries
//! inspect authorization; they do not activate camera, microphone or location.
use glory::blitz::permissions::{Permission, Permissions};
use glory::prelude::*;

const KINDS: [(Permission, &str); 6] = [
    (Permission::Camera, "Camera"),
    (Permission::Microphone, "Microphone"),
    (Permission::PhotosRead, "Photos read"),
    (Permission::PhotosAdd, "Photos add"),
    (Permission::LocationWhenInUse, "Location"),
    (Permission::Notifications, "Notifications"),
];

pub(crate) fn fixture(service: Permissions) -> impl Widget {
    widget(move |scope| {
        let status = scope.cage("Querying permissions".to_owned());
        let pending = scope.cage(None::<glory::core::TaskToken>);
        let busy = scope.cage(false);
        let cleanup = service.clone();
        scope.owner().on_cleanup(move || cleanup.close())?;
        let query = service.clone();
        scope.owner().spawn(async move {
            let mut lines = Vec::new();
            for (kind, label) in KINDS {
                let result = query.query(kind).await;
                eprintln!("Glory permissions query {label}: {result:?}");
                lines.push(format!("{label}: {result:?}"));
            }
            status.revise(|value| *value = lines.join(" | "));
            eprintln!(
                "Glory permissions startup queries complete; {:?}; native={:?}",
                query.stats(),
                query.native_stats()
            );
        })?;
        let mut rows: Vec<Box<dyn Widget>> = Vec::new();
        for (kind, label) in KINDS {
            let request = service.clone();
            let weak = scope.downgrade();
            rows.push(Box::new(
                button()
                    .label(format!("Request {label}"))
                    .enabled(busy.map(|v| !v))
                    .style(Style {
                        font_size: Some(16.),
                        ..Style::new().padding(Edges::all(8.))
                    })
                    .on(activate, move |_| {
                        let Some(scope) = weak.upgrade() else {
                            return;
                        };
                        busy.revise(|v| *v = true);
                        status.revise(|v| *v = format!("Waiting for {label}"));
                        let request = request.clone();
                        match scope.owner().spawn(async move {
                            let result = request.request(kind).await;
                            eprintln!(
                                "Glory permissions request {label}: {result:?}; native={:?}",
                                request.native_stats()
                            );
                            status.revise(|v| *v = format!("{label}: {result:?}"));
                            busy.revise(|v| *v = false);
                            pending.revise(|v| *v = None);
                        }) {
                            Ok(token) => pending.revise(|v| *v = Some(token)),
                            Err(error) => {
                                busy.revise(|v| *v = false);
                                scope.report_error(error);
                            }
                        }
                    }),
            ));
        }
        let inspect = service.clone();
        col()
            .style(Style {
                font_size: Some(14.),
                ..Style::new().padding(Edges::all(24.)).gap(8.)
            })
            .fill((
                text().content("Permission Probe"),
                text().content("Checks authorization only; no images, audio, photos or coordinates are accessed."),
                text().content(status),
                col().style(Style::new().gap(8.)).fill(rows),
                button()
                    .label("Cancel result delivery")
                    .on(activate, move |_| {
                        if let Some(token) = pending.get() {
                            token.cancel();
                        }
                        pending.revise(|v| *v = None);
                        busy.revise(|v| *v = false);
                        status.revise(|v| {
                            *v = "Result delivery cancelled; system dialog may remain".to_owned()
                        });
                    }),
                button()
                    .label("Inspect permission jobs")
                    .on(activate, move |_| {
                        let value =
                            format!("{:?}; native={:?}", inspect.stats(), inspect.native_stats());
                        eprintln!("Glory permissions inspect: {value}");
                        status.revise(|v| *v = value);
                    }),
            ))
            .show_in(scope)
    })
}
