using System.Collections.Concurrent;

namespace SuperDuper.Windows.Infrastructure.Protocol;

internal sealed class ResponseCorrelator
{
    private readonly ConcurrentDictionary<string, TaskCompletionSource<ResponseEnvelope>> _pending = [];

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
        if (!_pending.TryRemove(response.Id, out var completion))
        {
            return false;
        }

        _ = completion.TrySetResult(response);
        return true;
    }

    public bool TryCancel(string id, CancellationToken cancellationToken) =>
        _pending.TryGetValue(id, out var completion) && completion.TrySetCanceled(cancellationToken);

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
    }
}
