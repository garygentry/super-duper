using System.Collections.Concurrent;

namespace SuperDuper.Windows.Infrastructure.Protocol;

internal sealed class ResponseCorrelator
{
    private readonly ConcurrentDictionary<string, TaskCompletionSource<ResponseEnvelope>> _pending = [];

    // A cancelled request's ID moves here instead of lingering in _pending: the caller has already
    // stopped waiting, but the worker was never told to stop the work, so its eventual response
    // must not be treated as an unknown request ID (WorkerClient.PumpStandardOutputAsync would
    // otherwise read that as a protocol violation and kill the worker).
    private readonly ConcurrentDictionary<string, byte> _cancelled = [];

    public Task<ResponseEnvelope> Register(string id)
    {
        var completion = new TaskCompletionSource<ResponseEnvelope>(
            TaskCreationOptions.RunContinuationsAsynchronously);

        if (!_pending.TryAdd(id, completion))
        {
            throw new InvalidOperationException($"Request ID {id} is already pending.");
        }

        return completion.Task;
    }

    public bool TryComplete(ResponseEnvelope response)
    {
        if (_pending.TryRemove(response.Id, out var completion))
        {
            _ = completion.TrySetResult(response);
            return true;
        }

        return _cancelled.TryRemove(response.Id, out _);
    }

    public bool TryCancel(string id, CancellationToken cancellationToken)
    {
        if (!_pending.TryRemove(id, out var completion))
        {
            return false;
        }

        _cancelled[id] = 0;
        return completion.TrySetCanceled(cancellationToken);
    }

    public bool TryFail(string id, Exception exception) =>
        _pending.TryRemove(id, out var completion) && completion.TrySetException(exception);

    public void FailAll(Exception exception)
    {
        foreach (var pair in _pending)
        {
            if (_pending.TryRemove(pair.Key, out var completion))
            {
                completion.TrySetException(exception);
            }
        }
        _cancelled.Clear();
    }
}
