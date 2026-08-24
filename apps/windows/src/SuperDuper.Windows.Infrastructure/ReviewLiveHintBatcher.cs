namespace SuperDuper.Windows.Infrastructure;

internal sealed record ReviewLiveHintBatch(
    long RunId,
    string RootPath,
    int EventCount,
    IReadOnlyList<string> Paths,
    bool Overflow);

internal sealed class ReviewLiveHintBatcher : IDisposable
{
    internal const int MaximumPathsPerBatch = 200;
    internal const int MaximumPendingRoots = 64;
    internal static readonly TimeSpan BatchInterval = TimeSpan.FromMilliseconds(100);

    private readonly long _runId;
    private readonly Func<TimeSpan, CancellationToken, Task> _delay;
    private readonly Func<ReviewLiveHintBatch, CancellationToken, Task> _sink;
    private readonly CancellationTokenSource _lifetime = new();
    private readonly object _gate = new();
    private readonly Dictionary<string, PendingRoot> _pending = new(StringComparer.OrdinalIgnoreCase);
    private readonly Queue<string> _order = new();
    private bool _pumpRunning;
    private bool _disposed;

    internal ReviewLiveHintBatcher(
        long runId,
        Func<ReviewLiveHintBatch, CancellationToken, Task> sink,
        Func<TimeSpan, CancellationToken, Task>? delay = null)
    {
        if (runId <= 0)
        {
            throw new ArgumentOutOfRangeException(nameof(runId));
        }
        _runId = runId;
        _sink = sink ?? throw new ArgumentNullException(nameof(sink));
        _delay = delay ?? Task.Delay;
    }

    internal void Enqueue(string rootPath, string path) =>
        Enqueue(rootPath, [path], 1, overflow: false);

    internal void EnqueueRename(string rootPath, string oldPath, string newPath) =>
        Enqueue(rootPath, [oldPath, newPath], 1, overflow: false);

    internal void EnqueueOverflow(string rootPath) =>
        Enqueue(rootPath, [], 1, overflow: true);

    private void Enqueue(
        string rootPath,
        IReadOnlyList<string> paths,
        int eventCount,
        bool overflow)
    {
        if (string.IsNullOrWhiteSpace(rootPath))
        {
            return;
        }
        lock (_gate)
        {
            if (_disposed)
            {
                return;
            }
            if (!_pending.TryGetValue(rootPath, out var root))
            {
                if (_pending.Count >= MaximumPendingRoots)
                {
                    return;
                }
                root = new PendingRoot(rootPath);
                _pending.Add(rootPath, root);
                _order.Enqueue(rootPath);
            }
            root.EventCount = Math.Min(int.MaxValue, root.EventCount + eventCount);
            if (overflow)
            {
                root.Overflow = true;
                root.Paths.Clear();
            }
            else if (!root.Overflow)
            {
                foreach (var path in paths)
                {
                    if (string.IsNullOrWhiteSpace(path))
                    {
                        continue;
                    }
                    if (!root.Paths.Contains(path)
                        && root.Paths.Count == MaximumPathsPerBatch)
                    {
                        root.Overflow = true;
                        root.Paths.Clear();
                        break;
                    }
                    root.Paths.Add(path);
                }
            }
            if (!_pumpRunning)
            {
                _pumpRunning = true;
                _ = PumpAsync(_lifetime.Token);
            }
        }
    }

    private async Task PumpAsync(CancellationToken cancellationToken)
    {
        try
        {
            while (true)
            {
                await _delay(BatchInterval, cancellationToken).ConfigureAwait(false);
                ReviewLiveHintBatch? batch;
                lock (_gate)
                {
                    batch = DrainNext();
                    if (batch is null)
                    {
                        _pumpRunning = false;
                        return;
                    }
                }
                try
                {
                    await _sink(batch, cancellationToken).ConfigureAwait(false);
                }
                catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
                {
                    return;
                }
                catch
                {
                    // Hints are not authoritative. Worker disconnect/recovery remains visible,
                    // and the durable overflow fallback is attempted by the production sink.
                }
            }
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
        }
        finally
        {
            lock (_gate)
            {
                _pumpRunning = false;
            }
        }
    }

    private ReviewLiveHintBatch? DrainNext()
    {
        while (_order.Count > 0)
        {
            var rootPath = _order.Dequeue();
            if (!_pending.Remove(rootPath, out var root))
            {
                continue;
            }
            return new ReviewLiveHintBatch(
                _runId,
                root.RootPath,
                root.EventCount,
                root.Paths.ToArray(),
                root.Overflow);
        }
        return null;
    }

    public void Dispose()
    {
        lock (_gate)
        {
            if (_disposed)
            {
                return;
            }
            _disposed = true;
            _pending.Clear();
            _order.Clear();
        }
        _lifetime.Cancel();
        _lifetime.Dispose();
    }

    private sealed class PendingRoot(string rootPath)
    {
        public string RootPath { get; } = rootPath;

        public HashSet<string> Paths { get; } = new(StringComparer.OrdinalIgnoreCase);

        public int EventCount { get; set; }

        public bool Overflow { get; set; }
    }
}
