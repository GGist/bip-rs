struct Scheduler;

impl Handler for Scheduler {
	type Timeout = ();
	type Message = ();
	
	fn timeout(&mut self, event_loop: &mut EventLoop<Scheduler>, timeout: ()) {
		
	}
}