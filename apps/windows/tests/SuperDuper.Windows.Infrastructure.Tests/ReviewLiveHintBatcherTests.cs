using SuperDuper.Windows.Infrastructure;

namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class ReviewLiveHintBatcherTests
{
    [TestMethod]
    public async Task MassBurstProducesOneBoundedBatchInsteadOfOneFramePerEvent()
    {
        var clock = new ManualDelay();
        var batches = new List<ReviewLiveHintBatch>();
        using var batcher = new ReviewLiveHintBatcher(
            7,
            (batch, _) =>
            {
                lock (batches)
                {
                    batches.Add(batch);
                }
                return Task.CompletedTask;
            },
            clock.DelayAsync);

        for (var index = 0; index < 1_000; index++)
        {
            batcher.Enqueue(@"C:\root", $@"C:\root\copy-{index % 20}.bin");
        }
        await clock.ReleaseNextAsync();
        await WaitUntilAsync(() => batches.Count == 1);

        Assert.AreEqual(1, batches.Count);
        Assert.AreEqual(1_000, batches[0].EventCount);
        Assert.AreEqual(20, batches[0].Paths.Count);
        Assert.IsFalse(batches[0].Overflow);
        Assert.AreEqual(ReviewLiveHintBatcher.BatchInterval, clock.Elapsed);
    }

    [TestMethod]
    public async Task CapacityOverflowUsesOneDurableFallbackBatch()
    {
        var clock = new ManualDelay();
        ReviewLiveHintBatch? observed = null;
        using var batcher = new ReviewLiveHintBatcher(
            8,
            (batch, _) =>
            {
                observed = batch;
                return Task.CompletedTask;
            },
            clock.DelayAsync);
        for (var index = 0; index <= ReviewLiveHintBatcher.MaximumPathsPerBatch; index++)
        {
            batcher.Enqueue(@"C:\root", $@"C:\root\mass-{index}.bin");
        }
        await clock.ReleaseNextAsync();
        await WaitUntilAsync(() => observed is not null);

        Assert.IsTrue(observed!.Overflow);
        Assert.AreEqual(201, observed.EventCount);
        Assert.AreEqual(0, observed.Paths.Count);
    }

    [TestMethod]
    public async Task GlobalDrainCannotExceedTenUiProducingBatchesPerSecond()
    {
        var clock = new ManualDelay();
        var batches = 0;
        using var batcher = new ReviewLiveHintBatcher(
            9,
            (_, _) =>
            {
                Interlocked.Increment(ref batches);
                return Task.CompletedTask;
            },
            clock.DelayAsync);

        for (var index = 0; index < 11; index++)
        {
            batcher.Enqueue($@"C:\root-{index}", $@"C:\root-{index}\copy.bin");
        }
        for (var expected = 1; expected <= 11; expected++)
        {
            await clock.ReleaseNextAsync();
            await WaitUntilAsync(() => Volatile.Read(ref batches) == expected);
        }

        Assert.AreEqual(11, batches);
        Assert.AreEqual(TimeSpan.FromMilliseconds(1_100), clock.Elapsed);
    }

    private static async Task WaitUntilAsync(Func<bool> predicate)
    {
        for (var attempt = 0; attempt < 1_000 && !predicate(); attempt++)
        {
            await Task.Delay(1);
        }
        Assert.IsTrue(predicate(), "The deterministic coalescer did not reach the expected state.");
    }

    private sealed class ManualDelay
    {
        private readonly object _gate = new();
        private readonly Queue<(TimeSpan Delay, TaskCompletionSource Completion)> _delays = new();

        public TimeSpan Elapsed { get; private set; }

        public Task DelayAsync(TimeSpan delay, CancellationToken cancellationToken)
        {
            var completion = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
            cancellationToken.Register(() => completion.TrySetCanceled(cancellationToken));
            lock (_gate)
            {
                _delays.Enqueue((delay, completion));
            }
            return completion.Task;
        }

        public async Task ReleaseNextAsync()
        {
            (TimeSpan Delay, TaskCompletionSource Completion) next;
            for (var attempt = 0; ; attempt++)
            {
                lock (_gate)
                {
                    if (_delays.Count > 0)
                    {
                        next = _delays.Dequeue();
                        Elapsed += next.Delay;
                        break;
                    }
                }
                Assert.IsTrue(attempt < 100, "The coalescer did not request its next bounded delay.");
                await Task.Yield();
            }
            next.Completion.TrySetResult();
        }
    }
}
