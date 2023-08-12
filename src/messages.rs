use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};

/// Simple single-producer multi-consumer message buffer, where receivers on a message and pull it
/// from the buffer.
pub struct Messages<T>(Arc<(Mutex<VecDeque<T>>, Condvar)>);
impl<T> Clone for Messages<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T> Default for Messages<T> {
    fn default() -> Self {
        Self(Arc::default())
    }
}
impl<T> Messages<T> {
    pub fn len(&self) -> usize {
        let v = self.0 .0.lock().unwrap();
        v.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn push(&self, message: T) {
        let mut v = self.0 .0.lock().unwrap();
        v.push_back(message);
        self.0 .1.notify_all();
    }
    pub fn wait(&self, filter: impl Fn(&T) -> bool) -> T {
        loop {
            let mut v = self
                .0
                 .1
                .wait_while(self.0 .0.lock().unwrap(), |v| !v.iter().any(&filter))
                .unwrap();
            if let Some((i, _)) = v.iter().enumerate().find(|(_, m)| filter(m)) {
                return v.swap_remove_back(i).unwrap();
            }
        }
    }
}
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn messages() {
        let messages = Messages::<u8>::default();
        let messages2 = messages.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            messages2.push(1);
            messages2.push(2);
            messages2.push(3);
        });
        assert_eq!(messages.wait(|x| *x == 2), 2);
        assert_eq!(messages.wait(|x| *x == 3), 3);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages.wait(|x| *x == 1), 1);
        assert_eq!(messages.len(), 0);
    }
}
