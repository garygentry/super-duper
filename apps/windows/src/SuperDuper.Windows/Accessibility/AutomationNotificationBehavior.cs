using System.Windows;
using System.Windows.Automation;
using System.Windows.Automation.Peers;
using System.Windows.Threading;

namespace SuperDuper.Windows.Accessibility;

public static class AutomationNotificationBehavior
{
    public static readonly DependencyProperty AnnouncementVersionProperty = DependencyProperty.RegisterAttached(
        "AnnouncementVersion",
        typeof(long),
        typeof(AutomationNotificationBehavior),
        new PropertyMetadata(0L, OnAnnouncementVersionChanged));

    public static readonly DependencyProperty NotificationKindProperty = DependencyProperty.RegisterAttached(
        "NotificationKind",
        typeof(AutomationNotificationKind),
        typeof(AutomationNotificationBehavior),
        new PropertyMetadata(AutomationNotificationKind.ActionCompleted));

    public static readonly DependencyProperty NotificationProcessingProperty = DependencyProperty.RegisterAttached(
        "NotificationProcessing",
        typeof(AutomationNotificationProcessing),
        typeof(AutomationNotificationBehavior),
        new PropertyMetadata(AutomationNotificationProcessing.MostRecent));

    public static readonly DependencyProperty ActivityIdProperty = DependencyProperty.RegisterAttached(
        "ActivityId",
        typeof(string),
        typeof(AutomationNotificationBehavior),
        new PropertyMetadata(string.Empty));

    private static readonly DependencyProperty IsLoadedHandlerAttachedProperty = DependencyProperty.RegisterAttached(
        "IsLoadedHandlerAttached",
        typeof(bool),
        typeof(AutomationNotificationBehavior),
        new PropertyMetadata(false));

    internal static event Action<FrameworkElement, string, AutomationNotificationKind, AutomationNotificationProcessing, string>?
        NotificationRaised;

    public static void SetAnnouncementVersion(DependencyObject element, long value) =>
        element.SetValue(AnnouncementVersionProperty, value);

    public static long GetAnnouncementVersion(DependencyObject element) =>
        (long)element.GetValue(AnnouncementVersionProperty);

    public static void SetNotificationKind(DependencyObject element, AutomationNotificationKind value) =>
        element.SetValue(NotificationKindProperty, value);

    public static AutomationNotificationKind GetNotificationKind(DependencyObject element) =>
        (AutomationNotificationKind)element.GetValue(NotificationKindProperty);

    public static void SetNotificationProcessing(DependencyObject element, AutomationNotificationProcessing value) =>
        element.SetValue(NotificationProcessingProperty, value);

    public static AutomationNotificationProcessing GetNotificationProcessing(DependencyObject element) =>
        (AutomationNotificationProcessing)element.GetValue(NotificationProcessingProperty);

    public static void SetActivityId(DependencyObject element, string value) =>
        element.SetValue(ActivityIdProperty, value);

    public static string GetActivityId(DependencyObject element) =>
        (string)element.GetValue(ActivityIdProperty);

    private static void OnAnnouncementVersionChanged(
        DependencyObject dependencyObject,
        DependencyPropertyChangedEventArgs eventArgs)
    {
        if (dependencyObject is not FrameworkElement element
            || eventArgs.NewValue is not long version
            || version <= 0
            || Equals(eventArgs.OldValue, eventArgs.NewValue))
        {
            return;
        }

        ScheduleNotification(element, version);
    }

    private static void ScheduleNotification(FrameworkElement element, long version)
    {
        if (!element.IsLoaded)
        {
            if (!(bool)element.GetValue(IsLoadedHandlerAttachedProperty))
            {
                element.SetValue(IsLoadedHandlerAttachedProperty, true);
                element.Loaded += OnElementLoaded;
            }
            return;
        }

        _ = element.Dispatcher.BeginInvoke(
            DispatcherPriority.DataBind,
            new Action(() =>
            {
                if (element.IsLoaded && GetAnnouncementVersion(element) == version)
                {
                    RaiseNotification(element);
                }
            }));
    }

    private static void OnElementLoaded(object sender, RoutedEventArgs eventArgs)
    {
        if (sender is not FrameworkElement element)
        {
            return;
        }

        element.Loaded -= OnElementLoaded;
        element.SetValue(IsLoadedHandlerAttachedProperty, false);
        var version = GetAnnouncementVersion(element);
        if (version > 0)
        {
            ScheduleNotification(element, version);
        }
    }

    private static void RaiseNotification(FrameworkElement element)
    {
        var announcement = AutomationProperties.GetName(element);
        if (string.IsNullOrWhiteSpace(announcement))
        {
            return;
        }

        var peer = UIElementAutomationPeer.FromElement(element)
            ?? UIElementAutomationPeer.CreatePeerForElement(element);
        if (peer is null)
        {
            return;
        }

        var kind = GetNotificationKind(element);
        var processing = GetNotificationProcessing(element);
        var activityId = GetActivityId(element);
        peer.RaiseNotificationEvent(kind, processing, announcement, activityId);
        NotificationRaised?.Invoke(element, announcement, kind, processing, activityId);
    }
}
