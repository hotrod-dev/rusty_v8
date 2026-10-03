#ifndef HOTROD_MODULE_FEEDBACK_H_
#define HOTROD_MODULE_FEEDBACK_H_
#include "v8/src/objects/tagged.h"
namespace v8::internal {
class Isolate;
class SourceTextModule;
class FeedbackCell;
using HotrodModuleFeedbackProvider = Tagged<FeedbackCell> (*)(
    Isolate*, Tagged<SourceTextModule>, void*);
struct HotrodModuleFeedbackState {
  HotrodModuleFeedbackProvider provider;
  void* data;
};
HotrodModuleFeedbackState ExchangeHotrodModuleFeedback(
    HotrodModuleFeedbackState next);
}  // namespace v8::internal
#endif
