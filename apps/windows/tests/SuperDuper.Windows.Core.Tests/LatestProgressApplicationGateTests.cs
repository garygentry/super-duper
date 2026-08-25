using SuperDuper.Windows.Core.ViewModels;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class LatestProgressApplicationGateTests
{
    [TestMethod]
    public async Task ThousandFramesStayWithinTenApplicationsPerHalfOpenSecondAndPreserveLatest()
    {
        var scheduler = new ManualDelay();
        var applications = new List<(int Payload, TimeSpan At)>();
        using var gate = new LatestProgressApplicationGate<int>(
            payload =>
            {
                lock (applications)
                {
                    applications.Add((payload, scheduler.Elapsed));
                }
            },
            scheduler.ScheduleAsync);
        gate.BeginRun(41);

        var sequence = 0UL;
        for (var slot = 0; slot < 100; slot++)
        {
            for (var update = 0; update < 10; update++)
            {
                sequence++;
                Assert.AreEqual(
                    ProgressOfferResult.Accepted,
                    gate.Offer(Frame(41, sequence, sequence, sequence, (int)sequence)));
            }
            await WaitUntilAsync(() => scheduler.PendingCount == 1);
            await scheduler.ReleaseNextAsync();
            var expectedApplications = slot + 1;
            await WaitUntilAsync(() => applications.Count == expectedApplications);
        }

        Assert.AreEqual(1_000UL, sequence);
        Assert.AreEqual(100, applications.Count);
        Assert.AreEqual(1_000, applications[^1].Payload);
        foreach (var start in applications.Select(application => application.At))
        {
            var end = start + TimeSpan.FromSeconds(1);
            Assert.IsTrue(
                applications.Count(application => application.At >= start && application.At < end) <= 10,
                $"More than ten applications occurred in [{start}, {end}).");
        }
    }

    [TestMethod]
    public async Task InvalidNewerFramesCannotReplaceTheLatestValidPendingFrame()
    {
        var scheduler = new ManualDelay();
        var applications = new List<string>();
        using var gate = CreateGate(scheduler, applications);
        gate.BeginRun(7);

        Assert.AreEqual(ProgressOfferResult.Accepted, gate.Offer(Frame(7, 1, 1, 10, "first")));
        Assert.AreEqual(ProgressOfferResult.Accepted, gate.Offer(Frame(7, 2, 2, 20, "latest-valid")));
        Assert.AreEqual(
            ProgressOfferResult.CounterRegression,
            gate.Offer(Frame(7, 3, 3, 19, "regressing")));
        Assert.AreEqual(
            ProgressOfferResult.WrongRun,
            gate.Offer(Frame(8, 3, 3, 30, "wrong-run")));
        Assert.AreEqual(
            ProgressOfferResult.DuplicateOrOutOfOrderSequence,
            gate.Offer(Frame(7, 2, 3, 30, "duplicate-sequence")));
        Assert.AreEqual(
            ProgressOfferResult.DuplicateOrOutOfOrderSourceRevision,
            gate.Offer(Frame(7, 3, 2, 30, "duplicate-revision")));

        await scheduler.ReleaseNextAsync();
        await WaitUntilAsync(() => applications.Count == 1);

        CollectionAssert.AreEqual(new[] { "latest-valid" }, applications);
    }

    [TestMethod]
    public async Task RepeatedBeginForTheSameRunPreservesPendingStateAndAdmissionBaselines()
    {
        var scheduler = new ManualDelay();
        var applications = new List<string>();
        using var gate = CreateGate(scheduler, applications);
        gate.BeginRun(9);
        Assert.AreEqual(ProgressOfferResult.Accepted, gate.Offer(Frame(9, 5, 5, 50, "preserved")));

        gate.BeginRun(9);

        Assert.AreEqual(
            ProgressOfferResult.DuplicateOrOutOfOrderSequence,
            gate.Offer(Frame(9, 1, 1, 1, "reset-would-accept")));
        Assert.AreEqual(1, scheduler.PendingCount);
        await scheduler.ReleaseNextAsync();
        await WaitUntilAsync(() => applications.Count == 1);
        CollectionAssert.AreEqual(new[] { "preserved" }, applications);
    }

    [TestMethod]
    public async Task CounterVectorShapeAndEveryCounterAreValidatedAgainstPendingTruth()
    {
        var scheduler = new ManualDelay();
        var applications = new List<string>();
        using var gate = CreateGate(scheduler, applications);
        gate.BeginRun(12);

        Assert.AreEqual(
            ProgressOfferResult.Accepted,
            gate.Offer(Frame(12, 1, 1, [10, 20, 30], "valid")));
        Assert.AreEqual(
            ProgressOfferResult.InvalidCounterVector,
            gate.Offer(Frame(12, 2, 2, [10, 20], "wrong-shape")));
        Assert.AreEqual(
            ProgressOfferResult.CounterRegression,
            gate.Offer(Frame(12, 2, 2, [10, 19, 31], "middle-regressed")));

        await scheduler.ReleaseNextAsync();
        await WaitUntilAsync(() => applications.Count == 1);

        CollectionAssert.AreEqual(new[] { "valid" }, applications);
    }

    [TestMethod]
    public async Task CancellingIsStickyAndDropsAlreadyPendingRunningState()
    {
        var scheduler = new ManualDelay();
        var applications = new List<string>();
        using var gate = CreateGate(scheduler, applications);
        gate.BeginRun(20);

        Assert.AreEqual(ProgressOfferResult.Accepted, gate.Offer(Frame(20, 1, 1, 1, "running")));
        Assert.IsTrue(gate.MarkCancelling(20));
        Assert.AreEqual(
            ProgressOfferResult.RunningAfterCancelling,
            gate.Offer(Frame(20, 2, 2, 2, "revived-running")));
        Assert.AreEqual(
            ProgressOfferResult.Accepted,
            gate.Offer(Frame(20, 2, 2, 2, "cancelling", "cancelling")));

        await scheduler.ReleaseNextAsync();
        await WaitUntilAsync(() => applications.Count == 1);

        CollectionAssert.AreEqual(new[] { "cancelling" }, applications);
        Assert.IsFalse(gate.MarkCancelling(21));
    }

    [TestMethod]
    public async Task DelayedDispatcherExecutesOneClosureWithTheLatestPendingPayload()
    {
        var scheduler = new ManualDelay();
        var applications = new List<string>();
        using var gate = CreateGate(scheduler, applications);
        gate.BeginRun(25);

        for (ulong sequence = 1; sequence <= 1_000; sequence++)
        {
            Assert.AreEqual(
                ProgressOfferResult.Accepted,
                gate.Offer(Frame(25, sequence, sequence, sequence, $"payload-{sequence}")));
        }

        Assert.AreEqual(1, scheduler.PendingCount);
        await scheduler.ElapseNextAsync();
        Assert.AreEqual(1, scheduler.PostedCount);
        Assert.AreEqual(0, applications.Count);

        scheduler.ExecuteNextPosted();
        await WaitUntilAsync(() => applications.Count == 1);

        CollectionAssert.AreEqual(new[] { "payload-1000" }, applications);
    }

    [TestMethod]
    public async Task PostedClosureRechecksTerminalGenerationBeforeApplying()
    {
        var scheduler = new ManualDelay();
        var applications = new List<string>();
        using var gate = CreateGate(scheduler, applications);
        gate.BeginRun(26);
        Assert.AreEqual(ProgressOfferResult.Accepted, gate.Offer(Frame(26, 1, 1, 1, "stale")));

        await scheduler.ElapseNextAsync();
        Assert.AreEqual(1, scheduler.PostedCount);
        Assert.IsTrue(gate.MarkTerminal(26));
        scheduler.ExecuteNextPosted();
        await scheduler.DrainAsync();

        Assert.AreEqual(0, applications.Count);
    }

    [TestMethod]
    public async Task PostedRunningClosureCannotApplyAfterCancellingIsLatched()
    {
        var scheduler = new ManualDelay();
        var applications = new List<string>();
        using var gate = CreateGate(scheduler, applications);
        gate.BeginRun(27);
        Assert.AreEqual(ProgressOfferResult.Accepted, gate.Offer(Frame(27, 1, 1, 1, "stale-running")));

        await scheduler.ElapseNextAsync();
        Assert.AreEqual(1, scheduler.PostedCount);
        Assert.IsTrue(gate.MarkCancelling(27));
        scheduler.ExecuteNextPosted();
        Assert.AreEqual(0, applications.Count);
        Assert.AreEqual(
            ProgressOfferResult.RunningAfterCancelling,
            gate.Offer(Frame(27, 2, 2, 2, "revived-running")));
        Assert.AreEqual(
            ProgressOfferResult.Accepted,
            gate.Offer(Frame(27, 2, 2, 2, "accepted-cancelling", "cancelling")));
        await scheduler.ReleaseNextAsync();
        await WaitUntilAsync(() => applications.Count == 1);
        CollectionAssert.AreEqual(new[] { "accepted-cancelling" }, applications);
    }

    [TestMethod]
    public async Task NewOfferAtIdlePumpBoundaryStartsExactlyOneNextSchedule()
    {
        var scheduler = new ManualDelay();
        var applications = new List<string>();
        using var gate = CreateGate(scheduler, applications);
        gate.BeginRun(28);
        Assert.AreEqual(ProgressOfferResult.Accepted, gate.Offer(Frame(28, 1, 1, 1, "first")));
        await scheduler.ReleaseNextAsync();
        await WaitUntilAsync(() => applications.Count == 1 && scheduler.PendingCount == 0);

        Assert.AreEqual(ProgressOfferResult.Accepted, gate.Offer(Frame(28, 2, 2, 2, "second")));
        await WaitUntilAsync(() => scheduler.PendingCount == 1);
        Assert.AreEqual(1, scheduler.PendingCount);
        await scheduler.ReleaseNextAsync();
        await WaitUntilAsync(() => applications.Count == 2);

        CollectionAssert.AreEqual(new[] { "first", "second" }, applications);
    }

    [TestMethod]
    public async Task TerminalResetAndDisposeCancelPendingWorkAndRejectStaleFrames()
    {
        var scheduler = new ManualDelay();
        var applications = new List<string>();
        var gate = CreateGate(scheduler, applications);
        gate.BeginRun(30);
        Assert.AreEqual(ProgressOfferResult.Accepted, gate.Offer(Frame(30, 1, 1, 1, "terminal-pending")));

        Assert.IsTrue(gate.MarkTerminal(30));
        Assert.AreEqual(ProgressOfferResult.Terminal, gate.Offer(Frame(30, 2, 2, 2, "post-terminal")));
        await scheduler.ReleaseAllPendingAsync();
        Assert.AreEqual(0, applications.Count);

        gate.Reset();
        Assert.AreEqual(ProgressOfferResult.Inactive, gate.Offer(Frame(30, 1, 1, 1, "post-reset")));
        gate.BeginRun(31);
        Assert.AreEqual(ProgressOfferResult.Accepted, gate.Offer(Frame(31, 1, 1, 1, "dispose-pending")));
        gate.Dispose();
        Assert.AreEqual(ProgressOfferResult.Disposed, gate.Offer(Frame(31, 2, 2, 2, "post-dispose")));
        await scheduler.ReleaseAllPendingAsync();
        Assert.AreEqual(0, applications.Count);
    }

    private static LatestProgressApplicationGate<string> CreateGate(
        ManualDelay scheduler,
        List<string> applications) =>
        new(
            payload =>
            {
                lock (applications)
                {
                    applications.Add(payload);
                }
            },
            scheduler.ScheduleAsync);

    private static ProgressApplicationEnvelope<T> Frame<T>(
        long runId,
        ulong sequence,
        ulong sourceRevision,
        ulong counter,
        T payload,
        string status = "running") =>
        Frame(runId, sequence, sourceRevision, [counter], payload, status);

    private static ProgressApplicationEnvelope<T> Frame<T>(
        long runId,
        ulong sequence,
        ulong sourceRevision,
        IReadOnlyList<ulong> counters,
        T payload,
        string status = "running") =>
        new(runId, sequence, sourceRevision, status, counters, payload);

    private static async Task WaitUntilAsync(Func<bool> predicate)
    {
        for (var attempt = 0; attempt < 1_000 && !predicate(); attempt++)
        {
            await Task.Delay(1);
        }
        Assert.IsTrue(predicate(), "The deterministic progress gate did not reach the expected state.");
    }

    private sealed class ManualDelay
    {
        private readonly object _gate = new();
        private readonly Queue<Waiter> _waiters = new();
        private readonly Queue<PostedCallback> _posted = new();

        public TimeSpan Elapsed { get; private set; }

        public int PendingCount
        {
            get
            {
                lock (_gate)
                {
                    return _waiters.Count(waiter => !waiter.Completion.Task.IsCompleted);
                }
            }
        }

        public int PostedCount
        {
            get
            {
                lock (_gate)
                {
                    return _posted.Count;
                }
            }
        }

        public Task ScheduleAsync(
            TimeSpan delay,
            CancellationToken cancellationToken,
            Action callback)
        {
            var waiter = new Waiter(delay, cancellationToken, callback);
            waiter.Cancellation = cancellationToken.Register(
                () => waiter.Completion.TrySetCanceled(cancellationToken));
            lock (_gate)
            {
                _waiters.Enqueue(waiter);
            }
            return waiter.Completion.Task;
        }

        public async Task ReleaseNextAsync()
        {
            await ElapseNextAsync();
            ExecuteNextPosted();
        }

        public async Task ElapseNextAsync()
        {
            for (var attempt = 0; ; attempt++)
            {
                Waiter? waiter = null;
                lock (_gate)
                {
                    while (_waiters.Count > 0)
                    {
                        var candidate = _waiters.Dequeue();
                        if (!candidate.Completion.Task.IsCompleted)
                        {
                            waiter = candidate;
                            Elapsed += candidate.Delay;
                            break;
                        }
                        candidate.Cancellation.Dispose();
                    }
                }
                if (waiter is not null)
                {
                    lock (_gate)
                    {
                        _posted.Enqueue(new PostedCallback(waiter));
                    }
                    return;
                }
                Assert.IsTrue(attempt < 1_000, "The gate did not request its next bounded delay.");
                await Task.Yield();
            }
        }

        public void ExecuteNextPosted()
        {
            PostedCallback posted;
            lock (_gate)
            {
                Assert.IsTrue(_posted.Count > 0, "The scheduler has no posted dispatcher callback.");
                posted = _posted.Dequeue();
            }
            posted.Waiter.Cancellation.Dispose();
            if (!posted.Waiter.CancellationToken.IsCancellationRequested)
            {
                posted.Waiter.Callback();
                posted.Waiter.Completion.TrySetResult();
            }
        }

        public async Task ReleaseAllPendingAsync()
        {
            await DrainAsync();
        }

        public async Task DrainAsync()
        {
            for (var attempt = 0; attempt < 1_000; attempt++)
            {
                List<Waiter> waiters;
                List<PostedCallback> posted;
                lock (_gate)
                {
                    waiters = _waiters.ToList();
                    _waiters.Clear();
                    posted = _posted.ToList();
                    _posted.Clear();
                }
                foreach (var waiter in waiters)
                {
                    waiter.Cancellation.Dispose();
                    waiter.Completion.TrySetCanceled(waiter.CancellationToken);
                }
                foreach (var callback in posted)
                {
                    callback.Waiter.Cancellation.Dispose();
                    callback.Waiter.Completion.TrySetCanceled(callback.Waiter.CancellationToken);
                }
                if (waiters.Count == 0 && posted.Count == 0)
                {
                    return;
                }
                await Task.Yield();
            }
            Assert.Fail("The gate retained pending deterministic delays.");
        }

        private sealed class Waiter(
            TimeSpan delay,
            CancellationToken cancellationToken,
            Action callback)
        {
            public TimeSpan Delay { get; } = delay;

            public CancellationToken CancellationToken { get; } = cancellationToken;

            public Action Callback { get; } = callback;

            public TaskCompletionSource Completion { get; } =
                new(TaskCreationOptions.RunContinuationsAsynchronously);

            public CancellationTokenRegistration Cancellation { get; set; }
        }

        private sealed record PostedCallback(Waiter Waiter);
    }
}
