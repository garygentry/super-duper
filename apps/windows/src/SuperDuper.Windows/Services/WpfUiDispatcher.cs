using System.Windows.Threading;
using SuperDuper.Windows.Core.Services;

namespace SuperDuper.Windows.Services;

public sealed class WpfUiDispatcher(Dispatcher dispatcher) : IUiDispatcher
{
    public void Post(Action action)
    {
        if (dispatcher.CheckAccess())
        {
            action();
        }
        else
        {
            _ = dispatcher.BeginInvoke(action, DispatcherPriority.DataBind);
        }
    }
}
