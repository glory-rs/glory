// Minimal prefixes of the public Windows UIAutomationClient COM interfaces.
// The framework .NET UIA wrapper predates ControllerFor (30104); query the real
// OS IUIAutomationElement directly, without touching the application's AX tree.
using System;
using System.Runtime.InteropServices;

[ComImport, Guid("30cbe57d-d9d0-452a-ab13-7ac5ac4825ee"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface UiaClient {
    void CompareElements(); void CompareRuntimeIds(); void GetRootElement();
    void ElementFromHandle(IntPtr hwnd, out UiaElement element);
    void ElementFromPoint(); void GetFocusedElement(); void GetRootElementBuildCache();
    void ElementFromHandleBuildCache(); void ElementFromPointBuildCache(); void GetFocusedElementBuildCache();
    void CreateTreeWalker(); void ControlViewWalker(); void ContentViewWalker(); void RawViewWalker();
    void RawViewCondition(); void ControlViewCondition(); void ContentViewCondition(); void CreateCacheRequest();
    void CreateTrueCondition([MarshalAs(UnmanagedType.IUnknown)] out object condition);
}
[ComImport, Guid("d22108aa-8ac5-49a5-837b-37bbb3d7591e"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface UiaElement {
    void SetFocus();
    void GetRuntimeId([MarshalAs(UnmanagedType.SafeArray, SafeArraySubType=VarEnum.VT_I4)] out int[] runtimeId);
    void FindFirst();
    void FindAll(int scope, [MarshalAs(UnmanagedType.IUnknown)] object condition, out UiaElements elements);
    void FindFirstBuildCache(); void FindAllBuildCache(); void BuildUpdatedCache();
    void GetCurrentPropertyValue(int property, [MarshalAs(UnmanagedType.Struct)] out object value);
}
[ComImport, Guid("14314595-b4bc-4055-95f2-58f2e42c9855"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface UiaElements {
    void Length(out int length);
    void GetElement(int index, out UiaElement element);
}
public static class GloryUiaRelations {
    public static string[] ControlledRuntimeIds(int hwnd, string name) {
        var client = (UiaClient)Activator.CreateInstance(Type.GetTypeFromCLSID(new Guid("ff48dba4-60ef-4201-aa87-54103eef594e")));
        try {
            UiaElement window; client.ElementFromHandle(new IntPtr(hwnd), out window);
            try {
                object condition; client.CreateTrueCondition(out condition);
                try {
                    UiaElements nodes; window.FindAll(4, condition, out nodes);
                    try {
                        int count; nodes.Length(out count);
                        for (int i=0; i<count; i++) {
                            UiaElement node; nodes.GetElement(i, out node);
                            try {
                                object label; node.GetCurrentPropertyValue(30005, out label);
                                if (!String.Equals(label as string, name, StringComparison.Ordinal)) continue;
                                object relation; node.GetCurrentPropertyValue(30104, out relation);
                                if (relation == null) return new string[0];
                                var targets = relation as UiaElements;
                                if (targets == null) throw new InvalidOperationException("ControllerFor did not return IUIAutomationElementArray");
                                try {
                                    int length; targets.Length(out length);
                                    var ids = new string[length];
                                    for (int j=0; j<length; j++) {
                                        UiaElement target; targets.GetElement(j, out target);
                                        try { int[] id; target.GetRuntimeId(out id); ids[j] = String.Join(",",id); }
                                        finally { Marshal.ReleaseComObject(target); }
                                    }
                                    return ids;
                                } finally { Marshal.ReleaseComObject(targets); }
                            } finally { Marshal.ReleaseComObject(node); }
                        }
                        throw new InvalidOperationException("Owned disclosure control not found");
                    } finally { Marshal.ReleaseComObject(nodes); }
                } finally { Marshal.ReleaseComObject(condition); }
            } finally { Marshal.ReleaseComObject(window); }
        } finally { Marshal.ReleaseComObject(client); }
    }
}
