#include <sys/types.h>
#include "zygisk.hpp"

// Safe starting point: unload in every process until actual hooks are added.
class ExampleModule : public zygisk::ModuleBase {
public:
    void onLoad(zygisk::Api *api, JNIEnv *) override { api_ = api; }
    void preAppSpecialize(zygisk::AppSpecializeArgs *) override {
        api_->setOption(zygisk::Option::DLCLOSE_MODULE_LIBRARY);
    }
    void preServerSpecialize(zygisk::ServerSpecializeArgs *) override {
        api_->setOption(zygisk::Option::DLCLOSE_MODULE_LIBRARY);
    }
private:
    zygisk::Api *api_ = nullptr;
};
REGISTER_ZYGISK_MODULE(ExampleModule)
