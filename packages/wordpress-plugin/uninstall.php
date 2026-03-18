<?php
/**
 * Uninstall Appz Static Site Builder.
 *
 * Removes all plugin data when uninstalled via WordPress admin.
 */

if ( ! defined( 'WP_UNINSTALL_PLUGIN' ) ) {
	exit;
}

delete_option( 'appz_sb_last_build_time' );
delete_option( 'appz_sb_last_build_status' );
