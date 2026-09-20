using SuperDuper.Windows.Infrastructure.Protocol;

namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class ResponseCorrelatorTests
{
    [TestMethod]
    public async Task TryCancel_RemovesThePendingEntryImmediately()
    {
        var correlator = new ResponseCorrelator();
        var responseTask = correlator.Register("1");

        Assert.IsTrue(correlator.TryCancel("1", CancellationToken.None));

        await Assert.ThrowsExactlyAsync<TaskCanceledException>(() => responseTask);
        // A second cancel finds nothing left to cancel: the entry is gone, not merely completed.
        Assert.IsFalse(correlator.TryCancel("1", CancellationToken.None));
    }

    [TestMethod]
    public void TryComplete_AcceptsALateResponseForAnAlreadyCancelledRequestWithoutFailing()
    {
        var correlator = new ResponseCorrelator();
        _ = correlator.Register("1");
        Assert.IsTrue(correlator.TryCancel("1", CancellationToken.None));

        // The worker was never told to stop the work, so its eventual answer must be accepted, not
        // read as an unknown request ID (which would otherwise fail every pending request and kill
        // the worker connection — see WorkerClient.PumpStandardOutputAsync).
        Assert.IsTrue(correlator.TryComplete(new ResponseEnvelope { Id = "1", Ok = true }));
        // Accepted once; a second late answer for the same ID really is unknown.
        Assert.IsFalse(correlator.TryComplete(new ResponseEnvelope { Id = "1", Ok = true }));
    }

    [TestMethod]
    public void TryComplete_RejectsAResponseForAnIdThatWasNeverRegistered()
    {
        var correlator = new ResponseCorrelator();
        Assert.IsFalse(correlator.TryComplete(new ResponseEnvelope { Id = "unknown", Ok = true }));
    }

    [TestMethod]
    public async Task FailAll_AlsoClearsCancelledIds()
    {
        var correlator = new ResponseCorrelator();
        var pending = correlator.Register("1");
        Assert.IsTrue(correlator.TryCancel("1", CancellationToken.None));
        await Assert.ThrowsExactlyAsync<TaskCanceledException>(() => pending);

        correlator.FailAll(new InvalidOperationException("connection lost"));

        // The cancelled-ID tolerance does not outlive a connection failure.
        Assert.IsFalse(correlator.TryComplete(new ResponseEnvelope { Id = "1", Ok = true }));
    }
}
