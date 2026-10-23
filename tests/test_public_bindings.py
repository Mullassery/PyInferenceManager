import pyinferencemanager


def test_native_result_types_are_exported():
    assert pyinferencemanager.BackendKind is not None
    assert pyinferencemanager.ExecutionPlan is not None
    assert pyinferencemanager.HardwareProfile is not None
    assert pyinferencemanager.WorkloadResult is not None
