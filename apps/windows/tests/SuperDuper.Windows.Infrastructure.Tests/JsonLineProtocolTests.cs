using System.Text.Json;
using SuperDuper.Windows.Infrastructure.Protocol;

namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class JsonLineProtocolTests
{
    [TestMethod]
    public void EncodeRequestFrame_WritesExactlyOneJsonLine()
    {
        var frame = JsonLineProtocol.EncodeRequestFrame(
            "request-7",
            "hello",
            new { protocolVersions = new[] { 1 } });

        Assert.IsTrue(frame.EndsWith('\n'));
        Assert.AreEqual(1, frame.Count(character => character == '\n'));
        Assert.IsFalse(frame.Contains('\r'));

        using var document = JsonDocument.Parse(frame[..^1]);
        Assert.AreEqual("request", document.RootElement.GetProperty("type").GetString());
        Assert.AreEqual("request-7", document.RootElement.GetProperty("id").GetString());
        Assert.AreEqual("hello", document.RootElement.GetProperty("method").GetString());
        Assert.AreEqual(
            1,
            document.RootElement.GetProperty("params").GetProperty("protocolVersions")[0].GetInt32());
    }

    [TestMethod]
    public async Task ResponseCorrelator_CompletesInterleavedResponsesById()
    {
        var correlator = new ResponseCorrelator();
        var first = correlator.Register("first");
        var second = correlator.Register("second");

        Assert.IsTrue(correlator.TryComplete(Success("second", 2)));
        Assert.AreEqual("second", (await second).Id);
        Assert.IsFalse(first.IsCompleted);

        Assert.IsTrue(correlator.TryComplete(Success("first", 1)));
        Assert.AreEqual("first", (await first).Id);
    }

    [TestMethod]
    public void ParseInboundFrame_PreservesTypedEventNameAndData()
    {
        const string line = """
            {"type":"event","event":"run.progress","data":{"runId":19,"sequence":4,"status":"running","phase":"hashing","filesDiscovered":1200,"bytesDiscovered":"9000","filesHashed":300,"warningCount":2}}
            """;

        var frame = JsonLineProtocol.ParseInboundFrame(line);

        Assert.IsNull(frame.Response);
        Assert.IsNotNull(frame.Event);
        Assert.AreEqual("run.progress", frame.Event.Name);
        Assert.AreEqual(19, frame.Event.Data.GetProperty("runId").GetInt64());
        Assert.AreEqual(4UL, frame.Event.Data.GetProperty("sequence").GetUInt64());
    }

    [TestMethod]
    public async Task ResponseCorrelator_AcceptsLateResponseAfterCallerCancellation()
    {
        var correlator = new ResponseCorrelator();
        using var source = new CancellationTokenSource();
        var pending = correlator.Register("cancelled");
        source.Cancel();

        Assert.IsTrue(correlator.TryCancel("cancelled", source.Token));
        await Assert.ThrowsExceptionAsync<TaskCanceledException>(async () => await pending);
        Assert.IsTrue(correlator.TryComplete(Success("cancelled", 1)));
    }

    private static ResponseEnvelope Success(string id, int value)
    {
        using var document = JsonDocument.Parse($"{{\"value\":{value}}}");
        return new ResponseEnvelope
        {
            Type = "response",
            Id = id,
            Ok = true,
            Result = document.RootElement.Clone(),
        };
    }
}
