use std::time::Duration;

use mio::{ event, Events, Interest, Poll, Token };

#[derive(Debug)]
pub struct PollManager {
  instance: Poll,
}

impl PollManager {
  pub fn new() -> Result<Self, std::io::Error> {
    let instance = Poll::new()?;

    Ok(PollManager { instance })
  }

  pub fn add<S>(
    &self,
    stream: &mut S,
    token: Token,
    interest: Interest
  ) -> Result<(), std::io::Error>
    where S: event::Source + ?Sized
  {
    self.instance.registry().register(stream, token, interest)?;
    Ok(())
  }
  pub fn update<S>(
    &self,
    stream: &mut S,
    token: Token,
    interest: Interest
  ) -> Result<(), std::io::Error>
    where S: event::Source + ?Sized
  {
    self.instance.registry().reregister(stream, token, interest)?;
    Ok(())
  }
  pub fn remove<S>(&self, stream: &mut S) -> Result<(), std::io::Error>
    where S: event::Source + ?Sized
  {
    self.instance.registry().deregister(stream)?;
    Ok(())
  }
  pub fn poll<'a>(&mut self, events: &'a mut Events, timeout: Option<Duration>) -> Result<&'a Events, std::io::Error> {
    self.instance.poll(events, timeout)?;
    Ok(events)
  }
  pub fn get(&self) -> &Poll {
    &self.instance
  }
}
