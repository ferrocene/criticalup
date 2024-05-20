use crate::handlers::handle_request;
use crate::Data;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use tiny_http::Server;

pub struct MockServer {
    data: Arc<Mutex<Data>>,
    server: Arc<Server>,
    handle: Option<JoinHandle<()>>,
    served_requests: Arc<AtomicUsize>,
}

impl MockServer {
    pub(crate) fn spawn(data: Data) -> Self {
        let data = Arc::new(Mutex::new(data));

        // Binding on port 0 results in the operative system picking a random available port,
        // without the need of generating a random port ourselves and validating the port is not
        // being used by another process.
        //
        // The real port can be then retrieved by checking the address of the bound server.
        let server = Arc::new(Server::http("127.0.0.1:0").unwrap());

        let served_requests = Arc::new(AtomicUsize::new(0));

        let data_clone = data.clone();
        let server_clone = server.clone();
        let served_requests_clone = served_requests.clone();
        let handle = std::thread::spawn(move || {
            server_thread(data_clone, server_clone, served_requests_clone);
        });

        Self {
            data,
            server,
            handle: Some(handle),
            served_requests,
        }
    }

    pub fn url(&self) -> String {
        format!("http://{}", self.server.server_addr())
    }

    pub fn served_requests_count(&self) -> usize {
        self.served_requests.load(Ordering::SeqCst)
    }

    pub fn edit_data(&self, f: impl FnOnce(&mut Data)) {
        f(&mut self.data.lock().unwrap());
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.server.unblock();
        if let Some(handle) = self.handle.take() {
            match handle.join() {
                Ok(_) => (),
                Err(err) => eprintln!("{err:?}"),
            }
        }
    }
}

fn server_thread(data: Arc<Mutex<Data>>, server: Arc<Server>, served_requests: Arc<AtomicUsize>) {
    for request in server.incoming_requests() {
        let response = handle_request(&data.lock().unwrap(), &request);
        request.respond(response).unwrap();

        served_requests.fetch_add(1, Ordering::SeqCst);
    }
}
