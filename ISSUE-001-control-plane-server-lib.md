# Issue #001: control-plane-server ライブラリの分離

## 目的

ControlPlane.cpp (WT Dev内蔵) のサーバーロジックを、ターミナルエミュレータ非依存の共通Rustライブラリとして切り出す。
これにより WT Dev / ghostty-win / 将来の任意のターミナルが同一品質のIPC機能を得る。

## アーキテクチャ

```
agent-ctl (クライアント) ← 既存、変更不要
    ↕ named pipe (プロトコル規約)
control-plane-server (共通ライブラリ) ← 新規作成
    ↕ TerminalProvider trait
各ターミナル実装 (WT / ghostty / etc.)
```

## TerminalProvider trait

```rust
pub trait TerminalProvider: Send + Sync {
    /// ターミナルバッファの末尾N行を取得
    fn read_buffer(&self, lines: usize) -> String;
    /// テキスト入力を注入 (raw=trueならブラケットペースト無し)
    fn send_input(&self, text: &[u8], raw: bool);
    /// タブ数を取得
    fn tab_count(&self) -> usize;
    /// アクティブタブのインデックス
    fn active_tab(&self) -> usize;
    /// タブを切り替え
    fn switch_tab(&mut self, index: usize);
    /// 新しいタブを開く
    fn new_tab(&mut self);
    /// タブを閉じる
    fn close_tab(&mut self, index: usize);
    /// ウィンドウタイトル
    fn title(&self) -> String;
    /// 作業ディレクトリ
    fn working_directory(&self) -> String;
    /// プロンプト行にいるかの推定
    fn at_prompt(&self) -> bool;
}
```

## ライブラリが担当するもの（ターミナル実装者が書かなくていい部分）

1. **named pipeサーバー** — CreateNamedPipe + ConnectNamedPipe + スレッド管理
2. **プロトコル解析** — `COMMAND|arg1|arg2` のパース
3. **レスポンス構築** — PONG, STATE, TAIL, ACK, AGENT_STATUS, ERR
4. **base64エンコード/デコード**
5. **セッションファイル管理** — 書き込み・削除
6. **AGENT_STATUS判定ロジック** — バッファ変化検知、IDLE/WORKING/APPROVAL判定、ms_since_change追跡
7. **承認検知** — "Allow once", "Action Required" パターンマッチ
8. **UIスレッドブロック対策** — 専用スレッドでバッファスナップショット取得、タイムアウト管理

## 実装タスク

### Task A: ライブラリスケルトン
- `~/control-plane-server/` にcargo init --lib
- TerminalProvider trait定義
- error.rs, protocol.rs (agent-ctlから移植)

### Task B: パイプサーバー
- ControlPlane.cppのthreadMain/handleClient/buildResponseをRustに移植
- TerminalProvider経由でバッファ取得・入力注入
- セッションファイル管理

### Task C: AGENT_STATUS判定エンジン
- バッファスナップショット差分検知
- consecutive idle判定
- 承認パターンマッチ
- エージェントタイプ別READY検知

### Task D: 統合テスト
- MockTerminalProviderでunit test
- agent-ctl → control-plane-server → MockProvider のE2Eテスト

## 参照
- `C:/Users/yuuji/WindowsTerminal/src/cascadia/TerminalApp/ControlPlane.cpp` — 移植元
- `~/agent-ctl/src/protocol.rs` — プロトコル定義（クライアント側）
- `~/agent-ctl/src/backend/wt.rs` — 現行WtBackend実装
