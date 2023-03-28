# stochastic-queue

A queue and MPMC channel that pops items in a uniformly random order, instead of FIFO. Useful for nondeterminism, such as simulating chaos for testing or picking branches unpredictably.

Because there is no predetermined order, it's not possible to peek or iterate the queue. An item will always be popped if at least one is present in the queue.

Check the [docs](https://docs.rs/stochastic-queue) for usage guides and examples.
