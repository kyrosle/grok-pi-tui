use pi_grok_adapter::{PiRpc, SpawnConfig};
use serde_json::json;
use std::time::Duration;

const HOST: &str = r#"
import json,os,sys,time
def record(text):
    with open(os.environ['PI_EOF_TRACE'],'a') as file: file.write(text+'\n')
record('start:'+','.join(sys.argv[1:]))
for line in sys.stdin:
    request=json.loads(line)
    print(json.dumps({'type':'response','id':request.get('id'),'command':request['type'],'success':True,'data':{'args':sys.argv[1:]}}),flush=True)
record('shutdown')
time.sleep(0.04)
record('disposed')
sys.exit(int(os.environ.get('PI_EOF_EXIT','0')))
"#;

#[tokio::test]
async fn graceful_eof_finishes_old_scope_before_replacing_resource_arguments() {
    let directory = tempfile::tempdir().unwrap();
    let trace = directory.path().join("scope.log");
    let mut process = PiRpc::spawn(SpawnConfig {
        program: "python3".into(),
        prefix_args: vec!["-u".into(), "-c".into(), HOST.into()],
        cwd: directory.path().into(),
        pi_args: vec!["old".into()],
        env: vec![("PI_EOF_TRACE".into(), trace.display().to_string())],
    })
    .await
    .unwrap();
    assert_eq!(
        process
            .rpc
            .request(json!({"type":"get_state"}))
            .await
            .unwrap()["args"],
        json!(["--mode", "rpc", "old"])
    );
    process
        .rpc
        .shutdown_gracefully(Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(&trace).unwrap(),
        "start:--mode,rpc,old\nshutdown\ndisposed\n"
    );
    let event = process.events.recv().await.unwrap();
    assert_eq!(event["type"], "adapter_process_exit");
    assert_eq!(event["intentional"], true);
    assert_eq!(event["generation"], 1);
    process
        .rpc
        .respawn_with_args(vec!["new".into()])
        .await
        .unwrap();
    assert_eq!(
        process
            .rpc
            .request(json!({"type":"get_state"}))
            .await
            .unwrap()["args"],
        json!(["--mode", "rpc", "new"])
    );
    assert_eq!(
        std::fs::read_to_string(&trace).unwrap(),
        "start:--mode,rpc,old\nshutdown\ndisposed\nstart:--mode,rpc,new\n"
    );
    process.rpc.kill().await;
}

#[tokio::test]
async fn failed_graceful_disposal_is_not_acknowledged_as_restart_ready() {
    let directory = tempfile::tempdir().unwrap();
    let trace = directory.path().join("scope.log");
    let process = PiRpc::spawn(SpawnConfig {
        program: "python3".into(),
        prefix_args: vec!["-u".into(), "-c".into(), HOST.into()],
        cwd: directory.path().into(),
        pi_args: vec!["old".into()],
        env: vec![
            ("PI_EOF_TRACE".into(), trace.display().to_string()),
            ("PI_EOF_EXIT".into(), "7".into()),
        ],
    })
    .await
    .unwrap();
    process
        .rpc
        .request(json!({"type":"get_state"}))
        .await
        .unwrap();
    assert!(
        process
            .rpc
            .shutdown_gracefully(Duration::from_secs(2))
            .await
            .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(trace).unwrap(),
        "start:--mode,rpc,old\nshutdown\ndisposed\n"
    );
    process.rpc.kill().await;
}
