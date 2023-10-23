use super::Aviator;

pub struct UrlHashAviator {
    
}

impl UrlHashAviator {
    pub fn new() -> Self {
        Self {
            
        }
    }
}

impl Aviator for UrlHashAviator {
    fn push(&mut self, path: &str){

    }
    fn replace(&mut self, path: &str) {

    }
    fn back(&mut self){
        
    }
    fn forward(&mut self){
        
    }
    fn go(&mut self, delta: i32){
        
    }
    fn length(&self) -> i32{
        
    }
}

impl Enabler for UrlHashAviator {
    fn enable(self, truck: Rc<RefCell<Truck>>) {
        truck.inject(self);
    }
}