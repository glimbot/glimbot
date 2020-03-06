use circular_queue::CircularQueue;

pub struct RateLimiter {
    queue: CircularQueue<i64>
}