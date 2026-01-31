import 'dart:async';

import 'package:bluebubbles/database/models.dart';
import 'package:bluebubbles/services/services.dart';
import 'package:bluebubbles/utils/logger/logger.dart';
import 'package:bluebubbles/services/backend/queue/queue_impl.dart';
import 'package:get/get.dart';

OutgoingQueue outq = Get.isRegistered<OutgoingQueue>() ? Get.find<OutgoingQueue>() : Get.put(OutgoingQueue());

class OutgoingQueue extends Queue {

  @override
  Future<dynamic> prepItem(QueueItem _) async {
    assert(_ is OutgoingItem);
    final item = _ as OutgoingItem;

    switch (item.type) {
      case QueueType.sendMultipart:
      case QueueType.sendMessage:
        return await ah.prepMessage(item.chat, item.message, item.selected, item.reaction, clearNotificationsIfFromMe: !(item.customArgs?['notifReply'] ?? false));
      case QueueType.sendAttachment:
        return await ah.prepAttachment(item.chat, item.message);
      default:
        Logger.info("Unhandled queue event: ${item.type.name}");
        break;
    }
  }

  Future<T> handleSend<T>(Future<T> Function() process, Chat chat) {
    var timer = Timer(const Duration(seconds: 5), () {
      chat.sendProgress.value = .9;
    });
    var t = process();
    t.then((c) {
      timer.cancel();
      if (chat.sendProgress.value != 0) {
        chat.sendProgress.value = 1;
        Timer(const Duration(milliseconds: 500), () {
          chat.sendProgress.value = 0;
        });
      }
    }).catchError((c) {
      timer.cancel();
      if (chat.sendProgress.value != 0) {
        chat.sendProgress.value = 1;
        Timer(const Duration(milliseconds: 500), () {
          chat.sendProgress.value = 0;
        });
      }
    });
    return t;
  }

  @override
  Future<void> handleQueueItem(QueueItem _) async {
    assert(_ is OutgoingItem);
    final item = _ as OutgoingItem;

    switch (item.type) {
      case QueueType.sendMessage:
        // Route messages based on source (iMessage vs Google Messages)
        if (GmMessageRouter.isGmChat(item.chat)) {
          await handleSend(
            () => GmMessageRouter.sendMessage(
              chat: item.chat,
              message: item.message,
              selected: item.selected,
              reaction: item.reaction,
            ),
            item.chat,
          );
        } else {
          await handleSend(() => ah.sendMessage(item.chat, item.message, item.selected, item.reaction), item.chat);
        }
        break;
      case QueueType.sendAttachment:
        // Route attachments based on source (iMessage vs Google Messages)
        if (GmMessageRouter.isGmChat(item.chat)) {
          await handleSend(
            () => GmMessageRouter.sendAttachment(
              chat: item.chat,
              message: item.message,
              isAudio: item.customArgs?['audio'] ?? false,
            ),
            item.chat,
          );
        } else {
          await handleSend(() => ah.sendAttachment(item.chat, item.message, item.customArgs?['audio'] ?? false), item.chat);
        }
        break;
      default:
        Logger.info("Unhandled queue event: ${item.type.name}");
        break;
    }
  }
}