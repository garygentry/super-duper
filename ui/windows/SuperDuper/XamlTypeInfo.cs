// XamlHelper — reflection-based replacement for the IComponentConnector that
// the XAML compiler's pass 2 would normally generate to wire up x:Name fields
// to their visual tree counterparts.
//
// The IXamlMetadataProvider (App partial class, XamlMetaDataProvider, etc.) is
// now auto-generated in XamlTypeInfo.g.cs since pass 2 succeeds after removing
// XAML event handler attributes from all .xaml files.
//
// Remove this file when WinAppSDK ships a .NET 10-compatible XamlCompiler
// that also generates IComponentConnector (pass 2 Connect() method).

using System.Reflection;
using Microsoft.UI.Xaml;

namespace SuperDuper
{
    /// <summary>
    /// Replaces the IComponentConnector wiring that the XAML compiler's pass 2
    /// would normally generate. Call after InitializeComponent() in constructors.
    /// </summary>
    internal static class XamlHelper
    {
        /// <summary>
        /// Uses FindName + reflection to connect x:Name fields declared in .g.i.cs
        /// to their visual tree counterparts.
        /// </summary>
        /// <param name="target">The object containing the private fields (the page/control/window)</param>
        /// <param name="nameScope">The FrameworkElement that owns the XAML namescope (Content for Window, 'this' for Page/UserControl)</param>
        public static void ConnectNamedElements(object target, FrameworkElement nameScope)
        {
            foreach (var field in target.GetType().GetFields(
                BindingFlags.Instance | BindingFlags.NonPublic | BindingFlags.DeclaredOnly))
            {
                if (field.FieldType.IsValueType) continue;
                if (field.GetValue(target) != null) continue;

                var element = nameScope.FindName(field.Name);
                if (element != null && field.FieldType.IsInstanceOfType(element))
                {
                    field.SetValue(target, element);
                }
            }
        }
    }
}
