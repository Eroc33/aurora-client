use futures::{Future,Stream,Async,Poll};
use tokio_timer::{Timer,TimerError,Sleep};
use std::time::Duration;

pub trait StreamExt
    where Self: Sized
{
    fn rate_limited(self, delay: Duration, timer: Timer, skips: u64) -> RateLimitedStream<Self>;
}

impl<S: Stream + Sized> StreamExt for S
{
    fn rate_limited(self, delay: Duration, timer: Timer, skips: u64) -> RateLimitedStream<Self>
    {
        RateLimitedStream::new(self,delay,timer,skips)
    }
}

///Only runs the underlying stream at certain time intervals
pub struct RateLimitedStream<S>
{
    //Inner stream to limit
    inner: S,
    //The current sleep
    sleep: Option<Sleep>,
    //Time to sleep between each bout of polling the stream
    delay: Duration,
    //The timer to use for the sleep
    timer: Timer,
    ///Skips some initial limiting if this is positive (useful if folding
    /// and you want an initial value out immediately)
    delay_skips: u64,
}

impl <S> RateLimitedStream<S>{
    pub fn new(inner: S, delay: Duration, timer: Timer, skips: u64) -> Self
    {
        RateLimitedStream{
            inner: inner,
            delay: delay,
            timer: timer,
            sleep: None,
            delay_skips: skips,
        }
    }
}

impl<S> Stream for RateLimitedStream<S>
where S: Stream,
      S::Error: From<TimerError>
{
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error>
    {
        if self.delay_skips > 0 {
            if let Async::Ready(item) = self.inner.poll()? {
                self.delay_skips -= 1;
                return Ok(Async::Ready(item));
            }
        }

        if let Some(Ok(Async::Ready(_))) = self.sleep.as_mut().map(|ref mut s| s.poll()) {
            if let Async::Ready(item) = self.inner.poll()? {
                self.sleep = Some(self.timer.sleep(self.delay));
                return Ok(Async::Ready(item));
            }
        }

        if self.sleep.is_none() {
            self.sleep = Some(self.timer.sleep(self.delay));
        }

        Ok(Async::NotReady)
    }
}
