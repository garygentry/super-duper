namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class SingleInstanceGateTests
{
    [TestMethod]
    public void SecondInstanceActivatesTheFirstAndIsRefused()
    {
        var state = Path.Combine(Path.GetTempPath(), $"super-duper-gate-{Guid.NewGuid():N}");
        using var activated = new SemaphoreSlim(0);

        using (var first = SingleInstanceGate.TryAcquire(state, () => activated.Release()))
        {
            Assert.IsNotNull(first);

            var second = SingleInstanceGate.TryAcquire(state, () => Assert.Fail("The refused instance must not listen."));
            Assert.IsNull(second);
            Assert.IsTrue(activated.Wait(TimeSpan.FromSeconds(5)), "The first instance was not asked to activate.");

            // Spelled differently, same folder.
            Assert.IsNull(SingleInstanceGate.TryAcquire(state.ToLowerInvariant() + Path.DirectorySeparatorChar, () => { }));
            Assert.IsTrue(activated.Wait(TimeSpan.FromSeconds(5)));

            using var other = SingleInstanceGate.TryAcquire(state + "-other", () => { });
            Assert.IsNotNull(other, "A different state folder is a different instance.");
        }

        using var after = SingleInstanceGate.TryAcquire(state, () => { });
        Assert.IsNotNull(after, "Disposing the first instance must release the folder.");
    }

    [TestMethod]
    public void EventNameIsStableAndLocal()
    {
        var name = SingleInstanceGate.EventName(@"C:\Users\someone\AppData\Local\SuperDuper");
        Assert.AreEqual(name, SingleInstanceGate.EventName(@"c:\users\someone\appdata\local\superduper\"));
        StringAssert.StartsWith(name, @"Local\SuperDuper-");
    }
}
