using System;
using System.Management;
using System.IO;

namespace FanControl.OmenPlugin
{
    /// <summary>
    /// HP Omen BIOS WMI 通信层。
    /// 封装与 root\wmi\hpqBIntM 的底层交互，提供命令发送和连接管理。
    /// </summary>
    internal static class OmenBios
    {
        private const uint COMMAND = 0x20008;
        private static readonly byte[] SIGNATURE = new byte[] { 0x53, 0x45, 0x43, 0x55 };
        private const string BIOS_NAMESPACE = @"root\wmi";
        private const string BIOS_METHOD_CLASS = "hpqBIntM";

        private static ManagementObject _biosMethods;
        private static bool _initialized;

        /// <summary>
        /// 初始化 WMI 连接，带重试机制以应对开机时驱动延迟注册的情况。
        /// </summary>
        /// <returns>是否成功连接</returns>
        public static bool Initialize()
        {
            // 重试逻辑：处理 HP Omen WMI 驱动在开机时延迟注册的情况
            for (int i = 0; i < 3; i++)
            {
                try
                {
                    var scope = new ManagementScope(BIOS_NAMESPACE);
                    scope.Options.EnablePrivileges = true; // 启用必要的系统权限
                    scope.Connect();

                    var query = new SelectQuery("SELECT * FROM " + BIOS_METHOD_CLASS);
                    using (var searcher = new ManagementObjectSearcher(scope, query))
                    {
                        foreach (ManagementObject obj in searcher.Get())
                        {
                            _biosMethods = obj;
                            break;
                        }
                    }

                    if (_biosMethods != null)
                    {
                        OmenPlugin.Log($"Init OK (Attempt {i + 1})");
                        _initialized = true;
                        return true;
                    }

                    OmenPlugin.Log($"Init pending: hpqBIntM not found, retrying... ({i + 1}/3)");
                    System.Threading.Thread.Sleep(1000);
                }
                catch (Exception ex)
                {
                    OmenPlugin.Log($"Init error (Attempt {i + 1}): {ex.Message}");
                    if (i == 2) return false;
                    System.Threading.Thread.Sleep(1000);
                }
            }

            OmenPlugin.Log("Init failed: hpqBIntM not found after retries.");
            return false;
        }

        /// <summary>
        /// 关闭并释放 WMI 连接资源。
        /// </summary>
        public static void Close()
        {
            if (_initialized)
            {
                _biosMethods?.Dispose();
                _initialized = false;
            }
        }

        /// <summary>
        /// 向 BIOS 发送 WMI 命令。
        /// </summary>
        /// <param name="commandType">命令类型（如 0x10=读取风扇数量, 0x2D=读取转速, 0x2E=设置转速）</param>
        /// <param name="data">命令数据负载</param>
        /// <param name="outputSize">期望的输出数据大小（决定调用 hpqBIOSIntX）</param>
        /// <returns>BIOS 返回的数据，失败返回 null</returns>
        public static byte[] SendCommand(uint commandType, byte[] data, int outputSize)
        {
            string dataStr = data != null ? BitConverter.ToString(data) : "null";
            OmenPlugin.Log($"BIOS Cmd=0x{commandType:X2} Out={outputSize} Data=[{dataStr}]");
            try
            {
                var inData = new ManagementClass(BIOS_NAMESPACE, "hpqBDataIn", null).CreateInstance();
                inData["Command"] = COMMAND;
                inData["CommandType"] = commandType;
                inData["Sign"] = SIGNATURE;

                if (data != null && data.Length > 0)
                {
                    inData["hpqBData"] = data;
                    inData["Size"] = (uint)data.Length;
                }
                else
                {
                    inData["Size"] = (uint)0;
                }

                var methodParams = _biosMethods.GetMethodParameters("hpqBIOSInt" + outputSize);
                methodParams["InData"] = inData;

                var outParams = _biosMethods.InvokeMethod("hpqBIOSInt" + outputSize, methodParams, null);

                if (outParams != null)
                {
                    var outData = outParams["OutData"] as ManagementBaseObject;
                    if (outData != null)
                    {
                        var returnCode = Convert.ToInt32(outData["rwReturnCode"]);
                        if (returnCode == 0 && outputSize > 0)
                        {
                            var dataField = outData["Data"] as byte[];
                            if (dataField != null)
                            {
                                OmenPlugin.Log($"  Ret=0 Data=[{BitConverter.ToString(dataField)}]");
                                return dataField;
                            }
                        }
                        else if (returnCode != 0)
                        {
                            OmenPlugin.Log($"  Ret!={returnCode}");
                        }
                    }
                }
            }
            catch (Exception ex)
            {
                OmenPlugin.Log($"  Err: {ex.Message}");
            }
            return null;
        }
    }
}
