namespace SuperDuper.Windows.Core.Services;

public interface IUiDispatcher
{
    void Post(Action action);
}
