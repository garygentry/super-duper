using System.ComponentModel;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Views;

internal static class ServerSortInteraction
{
    public static WorkerSortDirection NextDirection<TField>(
        TField currentField,
        WorkerSortDirection currentDirection,
        TField requestedField)
        where TField : struct, Enum =>
        EqualityComparer<TField>.Default.Equals(currentField, requestedField)
            ? currentDirection == WorkerSortDirection.Ascending
                ? WorkerSortDirection.Descending
                : WorkerSortDirection.Ascending
            : WorkerSortDirection.Ascending;

    public static ListSortDirection ToListDirection(WorkerSortDirection direction) =>
        direction == WorkerSortDirection.Ascending
            ? ListSortDirection.Ascending
            : ListSortDirection.Descending;
}
