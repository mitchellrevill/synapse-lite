import contextlib
import types
import unittest
import importlib.util
from pathlib import Path

spec = importlib.util.spec_from_file_location("stock_snapshot", Path(__file__).with_name("stock-process-snapshot.py"))
snapshot = importlib.util.module_from_spec(spec)
spec.loader.exec_module(snapshot)


class Process:
    pid = 42
    def name(self): return "RzSDKService.exe"
    def exe(self): return r"C:\Vendor\SDK\bin\service.exe"
    def create_time(self): return 1_000_000.125
    def oneshot(self): return contextlib.nullcontext()
    def memory_info(self): return types.SimpleNamespace(rss=8192, private=16384)
    def cpu_times(self): return types.SimpleNamespace(user=2.5, system=1.25)


class SnapshotTests(unittest.TestCase):
    def capture(self, rows, fresh=None):
        return snapshot.capture([r"C:\Vendor\SDK"], rows, fresh or (lambda pid: Process()), (LookupError,))

    def test_native_counter_units_and_two_creation_observations(self):
        calls = []
        def fresh(pid): calls.append(pid); return Process()
        row = self.capture([Process()], fresh)["processes"][0]
        self.assertEqual(calls, [42])
        self.assertEqual((row["WorkingSet"], row["PrivateBytes"], row["CpuSeconds"]), (8192,16384,3.75))
        self.assertEqual(row["CreationDate"], row["StartUtc"])

    def test_literal_directory_boundary_and_case(self):
        self.assertTrue(snapshot.inside(r"c:\vendor\sdk\bin\app.exe", ["C:\\Vendor\\SDK\\"]))
        self.assertFalse(snapshot.inside(r"C:\Vendor\SDK-foreign\app.exe", [r"C:\Vendor\SDK"]))
        self.assertFalse(snapshot.inside(r"C:\Vendor\SDK\..\outside\app.exe", [r"C:\Vendor\SDK"]))

    def test_empty_or_outside_stock_never_becomes_zero_memory(self):
        with self.assertRaises(RuntimeError): self.capture([])
        row=Process(); row.exe=lambda:r"C:\Elsewhere\RazerHelper.exe"
        with self.assertRaises(RuntimeError): self.capture([row])

    def test_inaccessible_stock_fails_but_unrelated_images_are_counted_as_unknown(self):
        row=Process()
        def denied(): raise LookupError("protected")
        row.exe=denied
        with self.assertRaises(RuntimeError): self.capture([row])
        row.name=lambda:"unrelated.exe"
        result=self.capture([row,Process()])
        self.assertEqual(len(result["processes"]),1)
        self.assertEqual(result["inaccessible_unrelated_images"],1)

    def test_reused_exited_or_duplicate_process_refuses_the_frame(self):
        replaced=Process(); replaced.create_time=lambda:1_000_002.125
        with self.assertRaises(RuntimeError): self.capture([Process()],lambda pid:replaced)
        def missing(pid): raise LookupError("exited")
        with self.assertRaises(RuntimeError): self.capture([Process()],missing)
        with self.assertRaises(RuntimeError): self.capture([Process(),Process()])

    def test_unreadable_negative_or_nonfinite_counters_are_not_zero(self):
        for value in [-1,float("nan"),float("inf"),None]:
            row=Process(); row.memory_info=lambda:types.SimpleNamespace(rss=8192,private=value)
            with self.assertRaises(RuntimeError): self.capture([row])
        row=Process()
        def missing(): raise LookupError("counter unavailable")
        row.cpu_times=missing
        with self.assertRaises(RuntimeError): self.capture([row])


if __name__ == "__main__":
    unittest.main()
