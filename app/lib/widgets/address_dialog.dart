// 按地址连接（被控制的一方）和邀请一台电脑（控制的一方）共用的对话框：
// 输入地址 → 正在连接 → 成了就关掉（没配对时接着弹出配对码），没成就说清楚原因、可以重试。

import 'package:flutter/material.dart';

import '../l10n/l10n.dart';
import '../state/app_scope.dart';
import '../state/app_state.dart';
import '../state/dial_text.dart';
import '../theme/tokens.dart';
import 'surface.dart';

/// 弹出对话框。[invite] 为真时是控制端邀请对方，否则是被控端按地址连接。[initial] 预先填好的地址。
Future<void> showAddressDialog(BuildContext context, {required bool invite, String initial = ''}) async {
  final state = AppScope.read(context);
  await showGeneralDialog<void>(
    context: context,
    barrierDismissible: true,
    barrierLabel: invite ? context.l10n.dialInviteTitle : context.l10n.dialConnectTitle,
    barrierColor: Colors.black.withValues(alpha: 0.34),
    transitionDuration: const Duration(milliseconds: 320),
    pageBuilder: (context, _, _) => AddressDialog(invite: invite, initial: initial),
    transitionBuilder: (context, animation, _, child) {
      final curved = CurvedAnimation(parent: animation, curve: Motion.spring, reverseCurve: Curves.easeIn);
      return FadeTransition(
        opacity: CurvedAnimation(parent: animation, curve: Curves.easeOut),
        child: ScaleTransition(scale: Tween(begin: 0.94, end: 1.0).animate(curved), child: child),
      );
    },
  );
  state.clearDial();
}

class AddressDialog extends StatefulWidget {
  const AddressDialog({super.key, required this.invite, this.initial = ''});

  final bool invite;
  final String initial;

  @override
  State<AddressDialog> createState() => _AddressDialogState();
}

class _AddressDialogState extends State<AddressDialog> {
  late final _controller = TextEditingController(text: widget.initial);
  bool _closing = false;

  @override
  void initState() {
    super.initState();
    _controller.addListener(() => setState(() {}));
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _go(AppState state) {
    if (_controller.text.trim().isEmpty) return;
    state.dialAddress(_controller.text, invite: widget.invite);
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = SfColors.of(context);
    final dial = state.dial?.invite == widget.invite ? state.dial : null;
    final connecting = dial != null && !dial.done;
    final error = dial?.done == true ? dial?.error : null;
    // 成了：关掉对话框。没配对时后台接着弹出配对码。
    if (dial != null && dial.succeeded && !_closing) {
      _closing = true;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted) return;
        state.showToast(
          widget.invite ? context.l10n.dialInvited(dial.address) : context.l10n.dialConnected(dial.address),
        );
        Navigator.of(context).pop();
      });
    }
    final failure = error == null
        ? null
        : dialFailure(context.l10n, error, address: dial!.address, invite: widget.invite);
    final hint = widget.invite ? context.l10n.dialInviteHint : context.l10n.dialConnectHint;
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Material(
          color: c.card,
          shape: continuous(28),
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 520),
            child: SingleChildScrollView(
              padding: const EdgeInsets.fromLTRB(26, 24, 26, 24),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Semantics(
                              header: true,
                              child: Text(
                                widget.invite ? context.l10n.dialInviteTitle : context.l10n.dialConnectTitle,
                                style: const TextStyle(fontSize: 20, fontWeight: FontWeight.w700),
                              ),
                            ),
                            const SizedBox(height: 4),
                            Text(
                              widget.invite ? context.l10n.dialInviteSubtitle : context.l10n.dialConnectSubtitle,
                              style: TextStyle(fontSize: 14, color: c.muted),
                            ),
                          ],
                        ),
                      ),
                      SfIconButton(
                        icon: Icons.close_rounded,
                        tooltip: context.l10n.commonClose,
                        onPressed: () => Navigator.of(context).pop(),
                      ),
                    ],
                  ),
                  const SizedBox(height: 18),
                  Text(
                    context.l10n.dialAddress,
                    style: TextStyle(fontSize: 14, fontWeight: FontWeight.w700, color: c.muted),
                  ),
                  const SizedBox(height: 8),
                  TextField(
                    controller: _controller,
                    autofocus: true,
                    enabled: !connecting,
                    onSubmitted: (_) => _go(state),
                    style: const TextStyle(
                      fontFamily: monoFont,
                      fontFamilyFallback: monoFallback,
                      fontSize: 17,
                      fontWeight: FontWeight.w600,
                    ),
                    decoration: InputDecoration(
                      isDense: true,
                      hintText: context.l10n.dialExample(widget.invite ? '192.168.2.18' : '192.168.2.12'),
                      hintStyle: TextStyle(color: c.muted, fontWeight: FontWeight.w500),
                      filled: true,
                      fillColor: c.card,
                      contentPadding: const EdgeInsets.symmetric(horizontal: 14, vertical: 14),
                      enabledBorder: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(14),
                        borderSide: BorderSide(color: error != null ? c.danger : c.accent, width: 2),
                      ),
                      focusedBorder: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(14),
                        borderSide: BorderSide(color: error != null ? c.danger : c.accent, width: 2),
                      ),
                      disabledBorder: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(14),
                        borderSide: BorderSide(color: c.line, width: 2),
                      ),
                    ),
                  ),
                  const SizedBox(height: 8),
                  Text(hint, style: TextStyle(fontSize: 13, color: c.muted, height: 1.55)),
                  if (connecting) ...[
                    const SizedBox(height: 16),
                    Container(
                      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
                      decoration: ShapeDecoration(color: c.accentSoft, shape: continuous(16)),
                      child: Semantics(
                        liveRegion: true,
                        child: Row(
                          children: [
                            SizedBox.square(
                              dimension: 18,
                              child: CircularProgressIndicator(strokeWidth: 2.5, color: c.accent),
                            ),
                            const SizedBox(width: 12),
                            Expanded(
                              child: Text(
                                widget.invite
                                    ? context.l10n.dialInviting(dial.address)
                                    : context.l10n.dialConnecting(dial.address),
                                style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w600),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ],
                  if (failure != null) ...[
                    const SizedBox(height: 16),
                    Container(
                      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
                      decoration: ShapeDecoration(color: c.danger.withValues(alpha: 0.1), shape: continuous(16)),
                      child: Semantics(
                        liveRegion: true,
                        child: Row(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Icon(Icons.warning_amber_rounded, size: 20, color: c.danger),
                            const SizedBox(width: 12),
                            Expanded(
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Text(
                                    failure.title,
                                    style: TextStyle(fontSize: 15, fontWeight: FontWeight.w700, color: c.danger),
                                  ),
                                  const SizedBox(height: 4),
                                  Text(failure.body, style: const TextStyle(fontSize: 14, height: 1.55)),
                                ],
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ],
                  const SizedBox(height: 20),
                  Row(
                    mainAxisAlignment: MainAxisAlignment.end,
                    children: [
                      PillButton(
                        label: context.l10n.commonCancel,
                        soft: true,
                        onPressed: () => Navigator.of(context).pop(),
                      ),
                      const SizedBox(width: 10),
                      PillButton(
                        label: error != null
                            ? context.l10n.dialRetry
                            : (widget.invite ? context.l10n.dialInvite : context.l10n.dialConnect),
                        primary: true,
                        onPressed: state.online && !connecting && _controller.text.trim().isNotEmpty
                            ? () => _go(state)
                            : null,
                      ),
                    ],
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
