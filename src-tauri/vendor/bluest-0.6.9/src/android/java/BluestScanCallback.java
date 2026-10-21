package com.github.alexmoon.bluest.android;

import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothGatt;
import android.bluetooth.BluetoothGattCallback;
import android.bluetooth.BluetoothGattCharacteristic;
import android.bluetooth.BluetoothGattService;
import android.bluetooth.BluetoothManager;
import android.bluetooth.le.BluetoothLeScanner;
import android.bluetooth.le.ScanCallback;
import android.bluetooth.le.ScanResult;
import android.content.Context;
import android.app.Application;
import android.util.Log;
import android.os.Looper;


public class BluestScanCallback extends ScanCallback {
    private int id;

    public BluestScanCallback(int id) {
        this.id = id;
    }

    @Override
    public void onScanResult(int callbackType, ScanResult result) {
        super.onScanResult(callbackType, result);
        nativeOnScanResult(id, callbackType, result);
    }

    private static native void nativeOnScanResult(int id, int callbackType, ScanResult result);

    @Override
    public void onScanFailed(int errorCode) {
        super.onScanFailed(errorCode);
        nativeOnScanFailed(id, errorCode);
    }

    private static native void nativeOnScanFailed(int id, int errorCode);
}
