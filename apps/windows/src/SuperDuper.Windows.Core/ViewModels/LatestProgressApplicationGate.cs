using System.Runtime.CompilerServices;

[assembly: InternalsVisibleTo("SuperDuper.Windows.Core.Tests")]

namespace SuperDuper.Windows.Core.ViewModels;

internal sealed record ProgressApplicationEnvelope<T>(
    long RunId,
    ulong Sequence,
    ulong SourceRevision,
    string Status,
    IReadOnlyList<ulong> CumulativeCounters,
    T Payload);

internal enum ProgressOfferResult
{
    Accepted,
    Inactive,
    WrongRun,
    InvalidStatus,
    DuplicateOrOutOfOrderSequence,
    DuplicateOrOutOfOrderSourceRevision,
    InvalidCounterVector,
    CounterRegression,
    RunningAfterCancelling,
    Terminal,
    Disposed,
}

internal delegate Task ProgressApplicationScheduler(
    TimeSpan delay,
    CancellationToken cancellationToken,
    Action callback);

/// <summary>
/// Admits cumulative progress defensively and applies only the newest admitted payload in each
/// bounded slot. The scheduler's task must complete only after its callback executes (or is
/// cancelled). There is therefore at most one outstanding dispatcher closure, and that closure
/// reads pending state at execution time instead of capturing a stale payload when it is posted.
/// </summary>
internal sealed class LatestProgressApplicationGate<T> : IDisposable
{
    internal static readonly TimeSpan ApplicationInterval = TimeSpan.FromMilliseconds(100);

    private readonly object _gate = new();
    private readonly ProgressApplicationScheduler _schedule;
    private readonly Action<T> _apply;
    private CancellationTokenSource _lifetime = new();
    private long _generation;
    private object? _pumpOwner;
    private long _runId;
    private ulong _lastAdmittedSequence;
    private ulong _lastAdmittedSourceRevision;
    private ulong[]? _lastAdmittedCounters;
    private PendingApplication? _pending;
    private bool _active;
    private bool _cancelling;
    private bool _terminal;
    private bool _disposed;

    internal LatestProgressApplicationGate(
        Action<T> apply,
        ProgressApplicationScheduler schedule)
    {
        _apply = apply ?? throw new ArgumentNullException(nameof(apply));
        _schedule = schedule ?? throw new ArgumentNullException(nameof(schedule));
    }

    internal void BeginRun(long runId)
    {
        if (runId <= 0)
        {
            throw new ArgumentOutOfRangeException(nameof(runId));
        }

        CancellationTokenSource previous;
        lock (_gate)
        {
            ObjectDisposedException.ThrowIf(_disposed, this);
            if (_runId == runId && (_active || _terminal))
            {
                return;
            }
            previous = ReplaceLifetimeLocked();
            _runId = runId;
            _lastAdmittedSequence = 0;
            _lastAdmittedSourceRevision = 0;
            _lastAdmittedCounters = null;
            _pending = null;
            _active = true;
            _cancelling = false;
            _terminal = false;
        }
        CancelAndDispose(previous);
    }

    internal ProgressOfferResult Offer(ProgressApplicationEnvelope<T> envelope)
    {
        lock (_gate)
        {
            if (_disposed)
            {
                return ProgressOfferResult.Disposed;
            }
            if (_terminal)
            {
                return ProgressOfferResult.Terminal;
            }
            if (!_active)
            {
                return ProgressOfferResult.Inactive;
            }
            if (envelope.RunId != _runId)
            {
                return ProgressOfferResult.WrongRun;
            }

            var isRunning = string.Equals(envelope.Status, "running", StringComparison.Ordinal);
            var isCancelling = string.Equals(envelope.Status, "cancelling", StringComparison.Ordinal);
            if (!isRunning && !isCancelling)
            {
                return ProgressOfferResult.InvalidStatus;
            }
            if (envelope.Sequence == 0 || envelope.Sequence <= _lastAdmittedSequence)
            {
                return ProgressOfferResult.DuplicateOrOutOfOrderSequence;
            }
            if (envelope.SourceRevision == 0
                || envelope.SourceRevision <= _lastAdmittedSourceRevision)
            {
                return ProgressOfferResult.DuplicateOrOutOfOrderSourceRevision;
            }
            if (_cancelling && isRunning)
            {
                return ProgressOfferResult.RunningAfterCancelling;
            }
            if (envelope.CumulativeCounters is null || envelope.CumulativeCounters.Count == 0)
            {
                return ProgressOfferResult.InvalidCounterVector;
            }

            var counters = envelope.CumulativeCounters.ToArray();
            if (_lastAdmittedCounters is not null)
            {
                if (counters.Length != _lastAdmittedCounters.Length)
                {
                    return ProgressOfferResult.InvalidCounterVector;
                }
                for (var index = 0; index < counters.Length; index++)
                {
                    if (counters[index] < _lastAdmittedCounters[index])
                    {
                        return ProgressOfferResult.CounterRegression;
                    }
                }
            }

            _lastAdmittedSequence = envelope.Sequence;
            _lastAdmittedSourceRevision = envelope.SourceRevision;
            _lastAdmittedCounters = counters;
            _cancelling |= isCancelling;
            _pending = new PendingApplication(envelope.Status, envelope.Payload);
            EnsurePumpLocked();
            return ProgressOfferResult.Accepted;
        }
    }

    internal bool MarkCancelling(long runId)
    {
        CancellationTokenSource? previous = null;
        lock (_gate)
        {
            if (_disposed || !_active || _terminal || runId != _runId)
            {
                return false;
            }
            _cancelling = true;
            previous = ReplaceLifetimeLocked();
            if (_pending is { Status: "running" })
            {
                _pending = null;
            }
            if (_pending is not null)
            {
                EnsurePumpLocked();
            }
        }
        CancelAndDispose(previous);
        return true;
    }

    internal bool MarkTerminal(long runId)
    {
        CancellationTokenSource? previous = null;
        lock (_gate)
        {
            if (_disposed || !_active || _terminal || runId != _runId)
            {
                return false;
            }
            previous = ReplaceLifetimeLocked();
            _pending = null;
            _active = false;
            _terminal = true;
        }
        CancelAndDispose(previous);
        return true;
    }

    internal void Reset()
    {
        CancellationTokenSource? previous = null;
        lock (_gate)
        {
            if (_disposed)
            {
                return;
            }
            previous = ReplaceLifetimeLocked();
            _runId = 0;
            _lastAdmittedSequence = 0;
            _lastAdmittedSourceRevision = 0;
            _lastAdmittedCounters = null;
            _pending = null;
            _active = false;
            _cancelling = false;
            _terminal = false;
        }
        CancelAndDispose(previous);
    }

    public void Dispose()
    {
        CancellationTokenSource? previous = null;
        lock (_gate)
        {
            if (_disposed)
            {
                return;
            }
            _disposed = true;
            previous = ReplaceLifetimeLocked();
            _pending = null;
            _active = false;
            _terminal = true;
        }
        CancelAndDispose(previous);
        _lifetime.Dispose();
    }

    private CancellationTokenSource ReplaceLifetimeLocked()
    {
        var previous = _lifetime;
        _lifetime = new CancellationTokenSource();
        _generation++;
        _pumpOwner = null;
        return previous;
    }

    private void EnsurePumpLocked()
    {
        if (_pumpOwner is not null)
        {
            return;
        }
        var generation = _generation;
        var cancellationToken = _lifetime.Token;
        var owner = new object();
        _pumpOwner = owner;
        _ = PumpAsync(owner, generation, cancellationToken);
    }

    private async Task PumpAsync(
        object owner,
        long generation,
        CancellationToken cancellationToken)
    {
        try
        {
            while (true)
            {
                await _schedule(
                        ApplicationInterval,
                        cancellationToken,
                        () => ApplyLatest(generation, cancellationToken))
                    .ConfigureAwait(false);
                lock (_gate)
                {
                    if (_disposed
                        || !_active
                        || _terminal
                        || generation != _generation
                        || cancellationToken.IsCancellationRequested
                        || _pending is null)
                    {
                        ClearPumpLocked(owner);
                        return;
                    }
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
                ClearPumpLocked(owner);
            }
        }
    }

    private void ApplyLatest(long generation, CancellationToken cancellationToken)
    {
        T payload;
        lock (_gate)
        {
            if (_disposed
                || !_active
                || _terminal
                || generation != _generation
                || cancellationToken.IsCancellationRequested
                || _pending is null)
            {
                return;
            }
            payload = _pending.Payload;
            _pending = null;
            _apply(payload);
        }
    }

    private void ClearPumpLocked(object owner)
    {
        if (ReferenceEquals(_pumpOwner, owner))
        {
            _pumpOwner = null;
        }
    }

    private static void CancelAndDispose(CancellationTokenSource lifetime)
    {
        lifetime.Cancel();
        lifetime.Dispose();
    }

    private sealed record PendingApplication(string Status, T Payload);
}
