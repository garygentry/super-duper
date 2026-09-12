namespace SuperDuper.Windows.Core.Tests;

// Advances long scans in one step; timers deliver one current tick, never days of replayed ticks.
internal sealed class ManualProgressClock : TimeProvider
{
    private DateTimeOffset _utcNow = new(2026, 9, 12, 12, 0, 0, TimeSpan.Zero);
    private long _timestamp;
    private readonly List<ManualTimer> _timers = [];
    public override DateTimeOffset GetUtcNow() => _utcNow;
    public override long GetTimestamp() => _timestamp;
    public override long TimestampFrequency => TimeSpan.TicksPerSecond;
    public void AdjustUtc(TimeSpan delta) => _utcNow += delta;

    public void Advance(TimeSpan delta)
    {
        _utcNow += delta;
        _timestamp += delta.Ticks;
        foreach (var timer in _timers.ToArray()) timer.FireIfDue();
    }

    public override ITimer CreateTimer(TimerCallback callback, object? state, TimeSpan dueTime, TimeSpan period)
    {
        var timer = new ManualTimer(this, callback, state);
        _timers.Add(timer);
        timer.Change(dueTime, period);
        return timer;
    }

    private sealed class ManualTimer(ManualProgressClock clock, TimerCallback callback, object? state) : ITimer
    {
        private long? _due;
        private TimeSpan _period;
        private bool _disposed;
        public bool Change(TimeSpan dueTime, TimeSpan period)
        {
            if (_disposed) return false;
            _due = dueTime == Timeout.InfiniteTimeSpan ? null : clock._timestamp + dueTime.Ticks;
            _period = period;
            return true;
        }
        internal void FireIfDue()
        {
            if (_disposed || _due is not { } due || due > clock._timestamp) return;
            _due = _period == Timeout.InfiniteTimeSpan ? null : clock._timestamp + _period.Ticks;
            callback(state);
        }
        public void Dispose() { _disposed = true; _due = null; }
        public ValueTask DisposeAsync() { Dispose(); return ValueTask.CompletedTask; }
    }
}
