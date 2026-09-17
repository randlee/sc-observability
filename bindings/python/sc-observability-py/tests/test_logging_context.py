"""B.5 fixtures run against installed native packages, plus explicit custom failures."""
from __future__ import annotations

import asyncio
import contextvars
import dataclasses
import logging
from pathlib import Path
import subprocess
import sys
import threading

import pytest
from sc_observability import Err, LogEvent, LoggerConfig, LogQuery, Ok, TraceContext, create_logger, generated
from sc_observability.context import bind_context
from sc_observability.logging import HandlerDropCause, create_handler


def value(result):
    assert isinstance(result, Ok), result
    return result.value


@pytest.fixture
def logger(tmp_path):
    logger = value(create_logger(LoggerConfig(service="b5-tests", log_root=str(tmp_path), level="trace")))
    try:
        yield logger
    finally:
        value(logger.shutdown())


def event(**context):
    return LogEvent(level="info", target="python.context", action="context.test", **context)


def record(level=logging.INFO, message="hello %s", args=("world",)):
    return logging.LogRecord("python.handler", level, "fixture.py", 1, message, args, None)


def snapshot(logger):
    value(logger.flush())
    return value(logger.query(LogQuery(limit=1000))).events


@pytest.mark.parametrize("level,expected", [(-1,"trace"),(9,"trace"),(10,"debug"),(19,"debug"),(20,"info"),(29,"info"),(30,"warn"),(39,"warn"),(40,"error"),(50,"error")])
def test_real_handler_levels_and_selected_fields(logger, level, expected):
    handler=value(create_handler(logger, extra_fields=("selected","absent")))
    item=record(level);item.selected=23;item.secret="not-selected"
    assert handler.emit(item) is None
    result=value(handler.last_result());assert result.kind=="emitted" and result.admission.kind=="accepted"
    rows=snapshot(logger);row=next(row for row in rows if row.action=="python.log")
    assert row.level==expected and row.target=="python.handler" and row.message=="hello world"
    assert row.fields["selected"].value==23
    assert "absent" not in row.fields and "secret" not in row.fields
    assert row.fields["sc_observability.binding.language"].value=="python"
    handler.close()


def test_context_inactive_inheritance_overrides_and_lifo(logger):
    outer=value(bind_context(request_id="outer",correlation_id="shared"))
    assert value(outer.last_result()).kind=="idle"
    value(logger.log(event()))
    assert isinstance(outer.close(),Err)
    value(outer.enter())
    assert isinstance(outer.enter(),Err)
    inner=value(bind_context(request_id="inner"));value(inner.enter())
    assert isinstance(outer.close(),Err)
    value(logger.log(event()))
    value(logger.log(event(request_id="explicit")))
    value(inner.close());value(inner.close())
    assert isinstance(inner.enter(),Err)
    value(logger.log(event()))
    value(outer.close());value(logger.log(event()))
    rows=[row for row in snapshot(logger) if row.action=="context.test"]
    assert [row.request_id for row in rows]==[None,"inner","explicit","outer",None]
    assert [row.correlation_id for row in rows]==[None,"shared","shared","shared",None]


@pytest.mark.parametrize("context", [{"request_id":""},{"correlation_id":""},{"request_id":True},{"trace":TraceContext("bad","bad")},{"trace":"bad"}])
def test_invalid_context_before_activation(context):
    result=bind_context(**context)
    assert isinstance(result,Err) and result.error.kind=="validation"


def test_empty_context_and_stack_limit():
    scopes=[]
    try:
        for _ in range(64):
            scope=value(bind_context());value(scope.enter());scopes.append(scope)
        overflow=value(bind_context())
        result=overflow.enter();assert isinstance(result,Err)
        assert result.error.code==generated.SC_OBSERVABILITY_PY_CONTEXT_SCOPE_INVALID
        assert isinstance(overflow.close(),Err)
    finally:
        for scope in reversed(scopes):value(scope.close())


def test_async_task_isolation_child_ownership_and_explicit_thread_transfer(logger):
    async def run():
        parent=value(bind_context(request_id="parent"));value(parent.enter())
        async def child(label):
            assert isinstance(parent.close(),Err)
            value(logger.log(event()))
            child_scope=value(bind_context(request_id=label));value(child_scope.enter())
            await asyncio.sleep(0)
            value(logger.log(event()))
            value(child_scope.close())
        try:
            await asyncio.gather(child("a"),child("b"))
            outcomes=[]
            def worker():
                outcomes.append(parent.close())
                value(logger.log(event()))
                transferred=value(bind_context(request_id="transferred"));value(transferred.enter())
                try:value(logger.log(event()))
                finally:value(transferred.close())
            thread=threading.Thread(target=worker);thread.start();thread.join(timeout=5)
            assert not thread.is_alive() and isinstance(outcomes[0],Err)
        finally:value(parent.close())
    asyncio.run(run())
    identifiers=[row.request_id for row in snapshot(logger) if row.action=="context.test"]
    assert sorted(x for x in identifiers if x is not None)==["a","b","parent","parent","transferred"]
    assert identifiers.count(None)==1


def test_copied_context_reset_failure_preserves_scope_and_application_exception():
    scope=value(bind_context(request_id="original"));value(scope.enter())
    try:
        other=contextvars.copy_context()
        result=other.run(scope.close)
        assert isinstance(result,Err) and result.error.kind=="internal"
        with pytest.raises(ValueError,match="application"):
            try:raise ValueError("application")
            finally:
                result=other.run(scope.close)
                assert isinstance(result,Err)
    finally:value(scope.close())


class CustomBackend:
    def __init__(self,result):self.result=result;self.timeouts=[];self.events=[]
    def log(self,event):self.events.append(event);return self.result
    def flush(self,timeout_ms):self.timeouts.append(timeout_ms);return self.result


def failures():
    common=dict(at="1970-01-01T00:00:00Z",code="SC_TEST_FAILURE",message="original",remediation=generated.OutputRemediationRecoverable(steps=("first","second")))
    extras=dict(field="event",configured="info",requested="trace",available="info",operation="flush",received=2,remote_kind="future_variant")
    for name in dir(generated):
        cls=getattr(generated,name)
        if name.startswith("OutputFailure") and dataclasses.is_dataclass(cls) and name!="OutputFailureCounts":
            kwargs={f.name:common.get(f.name,extras.get(f.name)) for f in dataclasses.fields(cls) if f.init}
            yield cls(**kwargs)


@pytest.mark.parametrize("failure",list(failures()),ids=lambda value:value.kind)
def test_every_custom_failure_counts_once_preserves_diagnostics(failure):
    backend=CustomBackend(Err(failure));handler=value(create_handler(backend))
    initial=value(handler.health());assert set(initial.dropped_by_cause)==set(HandlerDropCause)
    assert sum(initial.dropped_by_cause.values())==0
    handler.emit(record())
    health=value(handler.health());assert isinstance(health.last_result,Err) and health.last_result.error is failure
    assert health.dropped_by_cause[HandlerDropCause(failure.kind)]==1
    assert sum(health.dropped_by_cause.values())==1
    handler.flush();assert backend.timeouts==[2000]
    assert sum(value(handler.health()).dropped_by_cause.values())==1
    assert sum(initial.dropped_by_cause.values())==0
    with pytest.raises(TypeError):health.dropped_by_cause[HandlerDropCause.INTERNAL]=10
    handler.close();handler.close()


def test_recursion_retained_and_foreign_formatting_failure(logger):
    handler=value(create_handler(logger))
    class Recursive(logging.LogRecord):
        calls=0
        def getMessage(self):
            self.calls+=1
            handler.emit(record(message="nested",args=()))
            return "outer"
    item=Recursive("python.handler",20,"fixture",1,"outer",(),None)
    handler.emit(item)
    assert item.calls==1
    result=handler.last_result();assert isinstance(result,Err)
    assert result.error.code==generated.SC_OBSERVABILITY_PY_HANDLER_REENTRANT
    assert result.error.message=="Recursive Python logging handler invocation"
    assert result.error.remediation.steps==("Remove logging calls from handler formatting and error callbacks",)
    health=value(handler.health());assert health.dropped_by_cause[HandlerDropCause.REENTRANT]==1 and health.dropped_by_cause[HandlerDropCause.INTERNAL]==0
    class Broken(logging.LogRecord):
        def getMessage(self):raise ValueError("foreign formatter")
    handler.emit(Broken("python.handler",20,"fixture",1,"bad",(),None))
    assert value(handler.health()).dropped_by_cause[HandlerDropCause.INTERNAL]==1
    handler._counts[HandlerDropCause.INTERNAL]=(1<<64)-1
    handler.emit(Broken("python.handler",20,"fixture",1,"bad",(),None))
    assert value(handler.health()).dropped_by_cause[HandlerDropCause.INTERNAL]==(1<<64)-1
    handler.close()


def test_filtered_closed_redaction_and_opt_in(logger):
    before=tuple(logging.getLogger().handlers)
    handler=value(create_handler(logger,extra_fields=("authorization",)))
    assert tuple(logging.getLogger().handlers)==before
    item=record(message="Bearer super-secret-token",args=());item.authorization="Bearer field-secret"
    try:raise ValueError("Bearer exception-secret")
    except ValueError:item.exc_info=sys.exc_info()
    handler.emit(item)
    row=next(row for row in snapshot(logger) if row.action=="python.log")
    assert "super-secret-token" not in row.message
    assert "field-secret" not in row.fields["authorization"].value
    assert "exception-secret" not in row.fields["exception"].value
    value(logger.shutdown());handler.emit(record())
    assert value(handler.health()).dropped_by_cause[HandlerDropCause.CLOSED]==1
    handler.close()


def test_filtered_is_success_without_drop(tmp_path):
    logger=value(create_logger(LoggerConfig(service="filter",log_root=str(tmp_path),level="error")))
    handler=value(create_handler(logger))
    try:
        handler.emit(record())
        assert value(handler.last_result()).admission.kind=="filtered"
        assert sum(value(handler.health()).dropped_by_cause.values())==0
        handler.close()
        assert value(logger.health()) is not None
    finally:value(logger.shutdown())


def test_handler_creation_validation_and_closed_resource(logger):
    for options in ({"level":True},{"extra_fields":["x"]},{"extra_fields":("",)}, {"extra_fields":("sc_observability::binding::language",)}):
        assert isinstance(create_handler(logger,**options),Err)
    assert isinstance(create_handler(object()),Err)
    handler=value(create_handler(logger));handler.close()
    handler.emit(record());assert value(handler.health()).dropped_by_cause[HandlerDropCause.CLOSED]==1
    handler.flush();assert isinstance(handler.last_result(),Err)
    value(logger.log(event()))  # handler close did not shut down the borrowed owner


def test_exception_formatter_failure_and_reentrant_outer_failure(logger):
    handler=value(create_handler(logger))
    class BrokenFormatter(logging.Formatter):
        def formatException(self,info):raise RuntimeError("foreign exception formatting")
    handler.setFormatter(BrokenFormatter())
    item=record()
    try:raise ValueError("application")
    except ValueError:item.exc_info=sys.exc_info()
    handler.emit(item)
    assert value(handler.health()).dropped_by_cause[HandlerDropCause.INTERNAL]==1
    class RecursiveFailure(logging.LogRecord):
        def getMessage(self):
            handler.emit(record())
            raise RuntimeError("later outer failure")
    handler.emit(RecursiveFailure("python.handler",20,"fixture",1,"bad",(),None))
    health=value(handler.health())
    assert health.dropped_by_cause[HandlerDropCause.REENTRANT]==1
    assert health.dropped_by_cause[HandlerDropCause.INTERNAL]==2
    assert isinstance(health.last_result,Err) and health.last_result.error.code==generated.SC_OBSERVABILITY_BINDING_INTERNAL
    handler.close()


def test_foreign_context_reset_failure_preserves_active_state(monkeypatch):
    import sc_observability.context as context
    scope=value(bind_context(request_id="live"));value(scope.enter())
    original=context._STACK
    class BrokenStack:
        def get(self):return original.get()
        def reset(self,token):raise RuntimeError("foreign reset failure")
    monkeypatch.setattr(context,"_STACK",BrokenStack())
    result=scope.close();assert isinstance(result,Err) and result.error.kind=="internal"
    monkeypatch.setattr(context,"_STACK",original)
    value(scope.close())


def test_trace_context_is_owned_and_restored(logger):
    trace=TraceContext("1"*32,"2"*16)
    scope=value(bind_context(trace=trace));value(scope.enter())
    try:
        value(logger.log(event()))
        value(logger.log(event(trace=TraceContext("3"*32,"4"*16))))
    finally:value(scope.close())
    rows=[row for row in snapshot(logger) if row.action=="context.test"]
    assert rows[0].trace.trace_id=="1"*32 and rows[1].trace.trace_id=="3"*32


def test_logging_shutdown_atexit_does_not_close_borrowed_logger(tmp_path):
    script="""
import logging,sys
from sc_observability import create_logger,LoggerConfig,Ok,LogEvent
from sc_observability.logging import create_handler
owner=create_logger(LoggerConfig(service='atexit',log_root=sys.argv[1]));assert isinstance(owner,Ok)
handler=create_handler(owner.value);assert isinstance(handler,Ok)
logging.getLogger('b5-explicit').addHandler(handler.value)
logging.shutdown()
assert isinstance(owner.value.log(LogEvent(level='info',target='still.live',action='after.handler.close')),Ok)
assert isinstance(owner.value.shutdown(),Ok)
print('B5_ATEXIT_OK')
"""
    result=subprocess.run([sys.executable,"-I","-c",script,str(tmp_path)],text=True,capture_output=True,timeout=10)
    assert result.returncode==0,result.stderr
    assert "B5_ATEXIT_OK" in result.stdout


def test_factory_foreign_input_inspection_is_contained(logger):
    class HostileTuple(tuple):
        def __iter__(self):
            raise RuntimeError("foreign tuple iterator")
    class HostileName(str):
        def __bool__(self):
            raise RuntimeError("foreign field inspection")
    class HostileBackend:
        def __getattribute__(self, name):
            raise RuntimeError("foreign backend inspection")
    class HostileTrace(TraceContext):
        def __getattribute__(self, name):
            raise RuntimeError("foreign trace snapshot")
    for result in (create_handler(None, extra_fields=HostileTuple(("x",))),
                   create_handler(logger, extra_fields=(HostileName("x"),)),
                   create_handler(HostileBackend()),
                   bind_context(trace=HostileTrace("1" * 32, "2" * 16))):
        assert isinstance(result, Err) and result.error.kind == "internal"


def test_accounting_failure_preserves_original_result(logger):
    handler = value(create_handler(logger))
    original = Err(generated.OutputFailureClosed(
        at="2025-01-01T00:00:00Z", code=generated.SC_OBSERVABILITY_BINDING_CLOSED,
        message="original", remediation=generated.OutputRemediationRecoverable(
            steps=("Stop submitting through the closed backend and inspect its retained health",))))
    class BrokenCounts(dict):
        def __setitem__(self, key, value):
            raise RuntimeError("foreign accounting fault")
    handler._counts = BrokenCounts(handler._counts)
    handler._record(original, event=True)
    assert handler.last_result() is original
    handler.close()


def test_concurrent_handler_calls_keep_exact_drop_counts():
    failure = next(item for item in failures() if item.kind == "queue_full")
    backend = CustomBackend(Err(failure))
    handler = value(create_handler(backend))
    barrier = threading.Barrier(5)
    errors = []
    def worker():
        try:
            barrier.wait(timeout=5)
            for _ in range(50):
                handler.emit(record())
        except BaseException as error:
            errors.append(error)
    threads = [threading.Thread(target=worker) for _ in range(4)]
    for thread in threads: thread.start()
    try:
        barrier.wait(timeout=5)
    finally:
        for thread in threads: thread.join(timeout=5)
    assert not errors and not any(thread.is_alive() for thread in threads)
    health = value(handler.health())
    assert health.dropped_by_cause[HandlerDropCause.QUEUE_FULL] == 200
    assert sum(health.dropped_by_cause.values()) == 200
    assert health.last_result.error is failure
    handler.close()


def test_stack_redaction_and_foreign_stack_formatter(logger):
    handler = value(create_handler(logger))
    item = record(); item.stack_info = "Bearer stack-secret"
    handler.emit(item)
    row = next(row for row in snapshot(logger) if row.action == "python.log")
    assert "stack-secret" not in row.fields["stack"].value
    class BrokenFormatter(logging.Formatter):
        def formatStack(self, stack_info):
            raise RuntimeError("foreign stack formatting")
    handler.setFormatter(BrokenFormatter()); handler.emit(item)
    assert value(handler.health()).dropped_by_cause[HandlerDropCause.INTERNAL] == 1
    handler.close()
