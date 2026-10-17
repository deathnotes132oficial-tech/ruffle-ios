// Helper to allow the storyboard to see the connections that we're making

#import <UIKit/UIKit.h>

@interface NavegacaoPrincipal : UINavigationController
@end

@interface PlayerView: UIView
@end

@interface PlayerController: UIViewController
@end

@interface LibraryController : UITableViewController
@property IBOutlet PlayerView* logoView;
- (IBAction) toggleEditing: (UIBarButtonItem*) sender;
- (IBAction) cancelEditItem: (UIStoryboardSegue*) segue;
- (IBAction) saveEditItem: (UIStoryboardSegue*) segue;
- (IBAction) showDocumentPicker: (id) sender;
@end

@interface EditController : UIViewController
@property IBOutlet UITableView* tableView;
@end
