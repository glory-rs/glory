use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use educe::Educe;

use crate::config::GloryConfig;
use crate::reflow::scheduler::{BATCHING, RUNING};
use crate::reflow::{PENDING_ITEMS, REVISING_ITEMS};
use crate::{Truck, Holder, HolderId, Widget, Scope, ViewId, ROOT_VIEWS};
use crate::web::widgets::Element;

const DEPOT_URL_KEY: &str = "glory::url";

pub struct ServerHolder {
    id: HolderId,
    pub config: Arc<GloryConfig>,
    pub truck: Rc<RefCell<Truck>>,
    pub host_node: Element,
    next_root_view_id: AtomicU64,
}

impl ServerHolder {
    pub fn new(config: impl Into<Arc<GloryConfig>>, url: impl Into<String>) -> Self {
        let mut truck = Truck::new();
        truck.insert(DEPOT_URL_KEY, url.into());
        Self {
            id: HolderId::next(),
            config: config.into(),
            truck: Rc::new(RefCell::new(truck)),
            host_node: crate::web::widgets::div(),
            next_root_view_id: AtomicU64::new(0),
        }
    }
}

impl Debug for ServerHolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerHolder").finish()
    }
}
impl Drop for ServerHolder {
    fn drop(&mut self) {
        ROOT_VIEWS.with_borrow_mut(|root_views| {
            root_views.remove(&self.id);
        });

        REVISING_ITEMS.with_borrow_mut(|revising_items| {
            revising_items.remove(&self.id);
        });
        PENDING_ITEMS.with_borrow_mut(|pending_items| {
            pending_items.remove(&self.id);
        });

        RUNING.with_borrow_mut(|runing| {
            runing.remove(&self.id);
        });
        BATCHING.with_borrow_mut(|batching| {
            batching.remove(&self.id);
        });
    }
}

impl Holder for ServerHolder {
    fn mount(self, widget: impl Widget) -> Self {
        let view_id = ViewId::new(self.id, self.next_root_view_id.fetch_add(1, Ordering::Relaxed).to_string());
        let scope = Scope::new_root(view_id, self.truck.clone());
        widget.mount_to(scope, self.host_node.node());
        self
    }
    fn truck(&self) -> Rc<RefCell<Truck>> {
        self.truck.clone()
    }
    // fn clone_boxed(&self) -> Box<dyn Holder> {
    //     Box::new(Self {
    //         truck: self.truck.clone(),
    //         host_node: self.host_node.clone(),
    //     })
    // }
}

cfg_feature! {
    #![feature = "salvo"]
    use std::convert::Infallible;

    use salvo::prelude::{Request, Response, Depot, FlowCtrl, Scribe, StatusCode};
    use salvo::{async_trait};

    impl Scribe for ServerHolder {
        fn render(self, res: &mut Response) {
            let (head, mid, tail) = crate::web::utils::html_parts_separated(&self.config, &*self.truck.borrow());
            res.add_header("content-type", "text/html", true).ok();
            res.status_code(StatusCode::OK);
            res.stream(futures::stream::iter([head, mid, self.host_node.node().inner_html(), tail.to_owned()].map(Result::<_, Infallible>::Ok)));
        }
    }

    #[derive(Clone, Educe)]
    #[educe(Debug)]
    pub struct SalvoHandler {
        pub config: Arc<GloryConfig>,
        #[educe(Debug(ignore))]
        pub holder_factory: Box<Arc<dyn Fn(Arc<GloryConfig>, String) -> ServerHolder +  Sync +Send + 'static>>,
    }

    // impl Clone for SalvoHandler {
    //     fn clone(&self) -> Self {
    //         Self {
    //             holder_factory: self.holder_factory.clone(),
    //         }
    //     }
    // }

    impl SalvoHandler {
        pub async fn new<H>(holder_factory: H ) -> Self where H: Fn(Arc<GloryConfig>, String) -> ServerHolder + Sync + Send + 'static{
            Self {config: Arc::new(GloryConfig::load(None).await.unwrap()),  holder_factory: Box::new(Arc::new(holder_factory)) }
        }
        pub fn config(mut self, config: impl Into<Arc<GloryConfig>>) -> Self {
            self.config = config.into();
            self
        }
    }

    #[async_trait]
    impl salvo::Handler for SalvoHandler {
        async fn handle(&self, req: &mut Request, _depot: &mut Depot, res: &mut Response, _ctrl: &mut FlowCtrl) {
            let holder = (self.holder_factory)(self.config.clone(), req.uri().to_string());
            res.render(holder);
        }
    }
}
